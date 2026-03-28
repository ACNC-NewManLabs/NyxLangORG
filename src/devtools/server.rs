use std::sync::{Arc, Mutex};
use std::net::SocketAddr;
use std::collections::HashMap;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures::{SinkExt, StreamExt};
use serde::{Serialize, Deserialize};
use serde_json;

use super::protocol::{DevtoolsEnvelope, DevtoolsStream, DevtoolsPayload};

#[derive(Debug, Clone, Default)]
pub struct DevtoolsServer {
    events: Arc<Mutex<Vec<DevtoolsEnvelope>>>,
}

impl DevtoolsServer {
    pub fn emit(&self, envelope: DevtoolsEnvelope) {
        if let Ok(mut events) = self.events.lock() {
            events.push(envelope);
        }
    }

    pub fn drain(&self) -> Vec<DevtoolsEnvelope> {
        if let Ok(mut events) = self.events.lock() {
            return std::mem::take(&mut *events);
        }
        Vec::new()
    }
}

// Enhanced DevTools server with WebSocket support
pub struct WebSocketDevtoolsServer {
    clients: Arc<Mutex<HashMap<String, WebSocketClient>>>,
    event_sender: UnboundedSender<DevtoolsEnvelope>,
    event_receiver: Arc<Mutex<UnboundedReceiver<DevtoolsEnvelope>>>,
    port: u16,
    running: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct WebSocketClient {
    id: String,
    sender: UnboundedSender<Message>,
    address: SocketAddr,
    connected_at: std::time::SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsRequest {
    pub id: String,
    pub method: String,
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsResponse {
    pub id: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<DevtoolsError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevtoolsError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub id: String,
    pub address: SocketAddr,
    pub connected_at: std::time::SystemTime,
    pub user_agent: Option<String>,
}

impl WebSocketDevtoolsServer {
    pub fn new(port: u16) -> Self {
        let (event_sender, event_receiver) = tokio::sync::mpsc::unbounded_channel();
        
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(Mutex::new(event_receiver)),
            port,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn start(&self) -> Result<(), DevtoolsServerError> {
        let addr = format!("127.0.0.1:{}", self.port);
        let listener = TcpListener::bind(&addr).await
            .map_err(|e| DevtoolsServerError::BindError(e.to_string()))?;

        println!("🔧 DevTools server listening on {}", addr);
        
        *self.running.lock().unwrap() = true;
        
        // Start event broadcasting task
        let clients_clone = self.clients.clone();
        let event_receiver_clone = self.event_receiver.clone();
        let running_clone = self.running.clone();
        
        tokio::spawn(async move {
            Self::broadcast_events(clients_clone, event_receiver_clone, running_clone).await;
        });

        // Accept connections
        while *self.running.lock().unwrap() {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let client_id = format!("client_{}", std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis());
                    
                    if let Err(e) = self.handle_connection(stream, addr, client_id).await {
                        eprintln!("Error handling connection: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }

    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    async fn handle_connection(
        &self,
        stream: TcpStream,
        addr: SocketAddr,
        client_id: String,
    ) -> Result<(), DevtoolsServerError> {
        let ws_stream = accept_async(stream).await
            .map_err(|e| DevtoolsServerError::WebSocketError(e.to_string()))?;

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (client_sender, client_receiver) = tokio::sync::mpsc::unbounded_channel();

        // Register client
        let client = WebSocketClient {
            id: client_id.clone(),
            sender: client_sender.clone(),
            address: addr,
            connected_at: std::time::SystemTime::now(),
        };

        self.clients.lock().unwrap().insert(client_id.clone(), client.clone());
        println!("🔗 Client connected: {} from {}", client_id, addr);

        // Send welcome message
        let welcome = DevtoolsEnvelope {
            seq: 1,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Inspector,
            payload: DevtoolsPayload::FrameStarted { frame_id: 0 },
        };

        let welcome_json = serde_json::to_string(&welcome)
            .map_err(|e| DevtoolsServerError::SerializationError(e.to_string()))?;
        
        client_sender.send(Message::Text(welcome_json.into()))
            .map_err(|e| DevtoolsServerError::SendError(e.to_string()))?;

        // Handle client messages
        let clients_clone = self.clients.clone();
        let client_id_clone = client_id.clone();
        
        tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Err(e) = Self::handle_client_message(&text, &client_id_clone, &clients_clone).await {
                            eprintln!("Error handling client message: {}", e);
                        }
                    }
                    Ok(Message::Close(_)) => {
                        println!("🔌 Client disconnected: {}", client_id_clone);
                        clients_clone.lock().unwrap().remove(&client_id_clone);
                        break;
                    }
                    Err(e) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Forward events to client
        let mut client_receiver = client_receiver;
        while let Some(event) = client_receiver.recv().await {
            if let Err(e) = ws_sender.send(event).await {
                eprintln!("Error sending to client: {}", e);
                break;
            }
        }

        // Clean up client
        self.clients.lock().unwrap().remove(&client_id);
        println!("🔌 Client removed: {}", client_id);

        Ok(())
    }

    async fn handle_client_message(
        message: &str,
        client_id: &str,
        clients: &Arc<Mutex<HashMap<String, WebSocketClient>>>,
    ) -> Result<(), DevtoolsServerError> {
        let request: DevtoolsRequest = serde_json::from_str(message)
            .map_err(|e| DevtoolsServerError::ParseError(e.to_string()))?;

        match request.method.as_str() {
            "Runtime.enable" => {
                // Enable runtime
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "runtimeId": "nyx-runtime"
                    })),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            "Page.enable" => {
                // Enable page
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "frameTree": {
                            "frame": {
                                "id": "main",
                                "url": "nyx://app",
                                "mimeType": "application/nyx"
                            }
                        }
                    })),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            "DOM.getDocument" => {
                // Get document
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({
                        "root": {
                            "nodeId": 1,
                            "nodeType": 9,
                            "nodeName": "#document",
                            "localName": "",
                            "nodeValue": "",
                            "children": [
                                {
                                    "nodeId": 2,
                                    "nodeType": 1,
                                    "nodeName": "HTML",
                                    "localName": "html",
                                    "nodeValue": "",
                                    "children": [
                                        {
                                            "nodeId": 3,
                                            "nodeType": 1,
                                            "nodeName": "BODY",
                                            "localName": "body",
                                            "nodeValue": "",
                                            "children": []
                                        }
                                    ]
                                }
                            ]
                        }
                    })),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            "Console.enable" => {
                // Enable console
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({})),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            "Network.enable" => {
                // Enable network
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({})),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            "Profiler.enable" => {
                // Enable profiler
                let response = DevtoolsResponse {
                    id: request.id,
                    result: Some(serde_json::json!({})),
                    error: None,
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
            
            _ => {
                // Unknown method
                let response = DevtoolsResponse {
                    id: request.id,
                    result: None,
                    error: Some(DevtoolsError {
                        code: -32601,
                        message: format!("Method '{}' not found", request.method),
                    }),
                };
                
                Self::send_response_to_client(clients, client_id, response).await?;
            }
        }

        Ok(())
    }

    async fn send_response_to_client(
        clients: &Arc<Mutex<HashMap<String, WebSocketClient>>>,
        client_id: &str,
        response: DevtoolsResponse,
    ) -> Result<(), DevtoolsServerError> {
        let response_json = serde_json::to_string(&response)
            .map_err(|e| DevtoolsServerError::SerializationError(e.to_string()))?;
        
        if let Some(client) = clients.lock().unwrap().get(client_id) {
            client.sender.send(Message::Text(response_json.into()))
                .map_err(|e| DevtoolsServerError::SendError(e.to_string()))?;
        }
        
        Ok(())
    }

    async fn broadcast_events(
        clients: Arc<Mutex<HashMap<String, WebSocketClient>>>,
        event_receiver: Arc<Mutex<UnboundedReceiver<DevtoolsEnvelope>>>,
        running: Arc<Mutex<bool>>,
    ) {
        while *running.lock().unwrap() {
            if let Ok(mut receiver) = event_receiver.try_lock() {
                while let Ok(event) = receiver.try_recv() {
                    let event_json = match serde_json::to_string(&event) {
                        Ok(json) => json,
                        Err(e) => {
                            eprintln!("Error serializing event: {}", e);
                            continue;
                        }
                    };

                    let clients_guard = clients.lock().unwrap();
                    for client in clients_guard.values() {
                        if let Err(e) = client.sender.send(Message::Text(event_json.clone().into())) {
                            eprintln!("Error broadcasting to client {}: {}", client.id, e);
                        }
                    }
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub fn get_connected_clients(&self) -> Vec<ClientInfo> {
        self.clients.lock().unwrap()
            .values()
            .map(|client| ClientInfo {
                id: client.id.clone(),
                address: client.address,
                connected_at: client.connected_at,
                user_agent: None,
            })
            .collect()
    }

    pub fn emit_event(&self, event: DevtoolsEnvelope) -> Result<(), DevtoolsServerError> {
        self.event_sender.send(event)
            .map_err(|e| DevtoolsServerError::SendError(e.to_string()))?;
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DevtoolsServerError {
    #[error("Bind error: {0}")]
    BindError(String),
    
    #[error("WebSocket error: {0}")]
    WebSocketError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Send error: {0}")]
    SendError(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Inspector frontend integration
pub struct InspectorFrontend {
    port: u16,
    server: WebSocketDevtoolsServer,
}

impl InspectorFrontend {
    pub fn new(port: u16) -> Self {
        let server = WebSocketDevtoolsServer::new(port);
        Self { port, server }
    }

    pub async fn start(&self) -> Result<(), DevtoolsServerError> {
        self.server.start().await
    }

    pub fn stop(&self) {
        self.server.stop();
    }

    pub fn get_inspector_url(&self) -> String {
        format!("http://127.0.0.1:{}/inspector.html", self.port)
    }

    pub fn emit_frame_started(&self, frame_id: u64) -> Result<(), DevtoolsServerError> {
        let event = DevtoolsEnvelope {
            seq: 1,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Timeline,
            payload: DevtoolsPayload::FrameStarted { frame_id },
        };
        
        self.server.emit_event(event)
    }

    pub fn emit_frame_ended(&self, frame_id: u64, total_micros: u64) -> Result<(), DevtoolsServerError> {
        let event = DevtoolsEnvelope {
            seq: 2,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Timeline,
            payload: DevtoolsPayload::FrameEnded { frame_id, total_micros },
        };
        
        self.server.emit_event(event)
    }

    pub fn emit_layout_stats(&self, nodes: usize, micros: u64) -> Result<(), DevtoolsServerError> {
        let event = DevtoolsEnvelope {
            seq: 3,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Timeline,
            payload: DevtoolsPayload::LayoutStats { nodes, micros },
        };
        
        self.server.emit_event(event)
    }

    pub fn emit_paint_stats(&self, ops: usize, micros: u64) -> Result<(), DevtoolsServerError> {
        let event = DevtoolsEnvelope {
            seq: 4,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Timeline,
            payload: DevtoolsPayload::PaintStats { ops, micros },
        };
        
        self.server.emit_event(event)
    }

    pub fn emit_hot_reload_patched(&self, modules: Vec<String>) -> Result<(), DevtoolsServerError> {
        let event = DevtoolsEnvelope {
            seq: 5,
            ts_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            stream: DevtoolsStream::Reload,
            payload: DevtoolsPayload::HotReloadPatched { modules },
        };
        
        self.server.emit_event(event)
    }
}
