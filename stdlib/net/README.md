# net_engine - Nyx Network Engine

A production-grade network engine for Nyx with async runtime, HTTP/1.1 server, WebSocket support, UDP, and more.

## Overview

The **net_engine** is a comprehensive networking framework for Nyx that provides:

- **Async Runtime**: Event loop, task scheduling, futures, and timers
- **HTTP Server**: Full HTTP/1.1 implementation with routing and middleware
- **WebSocket Server**: Real-time bidirectional communication
- **TCP/UDP**: Low-level socket programming support
- **Templates**: Dynamic HTML rendering
- **JSON**: Encoding/decoding utilities
- **Logging**: Structured logging with multiple levels
- **Security**: TLS/SSL support

### Performance Targets

- **100,000+ concurrent connections** via async I/O
- Low-latency request handling
- Efficient memory usage with connection pooling

---

## Architecture

### Module Structure

```
net_engine/
├── runtime/           # Async runtime foundation
│   ├── event_loop     # Event loop implementation
│   ├── task           # Task spawning and management
│   ├── future         # Future/promise handling
│   └── timer          # Timer and timeout support
├── tcp/               # TCP server and client
├── http/              # HTTP/1.1 server
│   ├── server         # HTTP server implementation
│   ├── request        # Request parsing
│   ├── response       # Response building
│   ├── router         # Route matching
│   ├── static         # Static file serving
│   └── middleware/   # HTTP middleware
│       ├── logger     # Request logging
│       ├── cors       # CORS headers
│       └── rate_limit # Rate limiting
├── websocket/         # WebSocket server
├── udp/               # UDP server
├── templates/        # Template engine
├── json/             # JSON utilities
├── logging/          # Logging framework
├── security/         # TLS/SSL
└── dev_server.nyx   # Development server
```

### Event-Driven Architecture

The net_engine uses an event-driven, non-blocking I/O model:

1. **Event Loop**: Central dispatcher that polls for I/O events
2. **Tasks**: Lightweight green threads spawned for concurrent operations
3. **Futures**: Represent asynchronous values that may not be ready yet
4. **Timers**: Schedule callbacks for future execution

---

## Installation & Setup

### Using the Engine in Nyx Projects

Import the net_engine in your Nyx application:

```nyx
import net.http;
import net.websocket;
import net.runtime;
```

### Engine Configuration

The engine is configured via [`engine.json`](engine.json):

```json
{
  "name": "net_engine",
  "entry": "./runtime/mod.nyx",
  "modules": [
    "./runtime/mod.nyx",
    "./http/mod.nyx",
    ...
  ],
  "version": "1.0.0",
  "description": "Production-grade network engine for Nyx"
}
```

---

## API Reference

### Async Runtime (`runtime/`)

The runtime module provides the foundation for asynchronous operations.

#### Initialization

```nyx
// Initialize the runtime
net.runtime.init_runtime();

// Start the event loop (blocks)
net.runtime.run();

// Stop gracefully
net.runtime.stop();
```

#### Task Management

```nyx
// Spawn a new async task
let task = net.runtime.spawn(fn() {
    print("Running in background");
});

// Await task completion
let result = net.runtime.await(task);
```

#### Timers

```nyx
// Sleep for 100ms
net.runtime.sleep(100);

// Set timeout (one-time)
net.runtime.set_timeout(1000, fn() {
    print("After 1 second");
});

// Set interval (repeating)
let timer = net.runtime.set_interval(500, fn() {
    print("Every 500ms");
});

// Cancel timer
net.runtime.clear_timer(timer);
```

#### Types

| Type | Description |
|------|-------------|
| [`runtime.EventLoop`](runtime/event_loop.nyx) | Event loop instance |
| [`runtime.Task`](runtime/task.nyx) | Task handle |
| [`runtime.Future`](runtime/future.nyx) | Future value |
| [`runtime.Timer`](runtime/timer.nyx) | Timer handle |

---

### TCP Server (`tcp/`)

Low-level TCP socket operations.

```nyx
import net.tcp;

// Create TCP server
let server = net.tcp.TcpServer(host="0.0.0.0", port=8080);

server.on_connect(fn(conn) {
    // Handle connection
    let data = conn.read();
    conn.write("Hello!");
    conn.close();
});

server.start();

// Or use shorthand
net.tcp.listen("0.0.0.0", 8080, fn(conn) {
    // Handle each connection
});
```

#### Types

| Function | Description |
|----------|-------------|
| [`tcp.TcpServer`](tcp/mod.nyx) | TCP server instance |
| [`tcp.TcpConnection`](tcp/server.nyx) | Connection handle |
| [`tcp.TcpClient`](tcp/mod.nyx) | TCP client |
| [`tcp.connect()`](tcp/mod.nyx:66) | Connect to TCP server |

---

### HTTP Server (`http/`)

Full-featured HTTP/1.1 server.

#### Basic Server

```nyx
import net.http;

// Create server on port 8080
let server = net.HttpServer(port: 8080);

// Add routes
server.get("/", fn(req, res) {
    res.html("<h1>Hello World!</h1>");
});

server.post("/api", fn(req, res) {
    res.json({"status": "ok"});
});

// Start server
server.start();
```

#### HTTP Methods

```nyx
server.get("/path", handler);      // GET
server.post("/path", handler);     // POST
server.put("/path", handler);      // PUT
server.delete("/path", handler);   // DELETE
server.patch("/path", handler);    // PATCH
```

#### Route Parameters

```nyx
server.get("/users/:id", fn(req, res) {
    let user_id = req.params["id"];
    res.json({"user_id": user_id});
});
```

#### Query Parameters

```nyx
server.get("/search", fn(req, res) {
    let query = req.query_params["q"] ?? "";
    let page = req.query_params["page"] ?? "1";
});
```

---

### HTTP Request (`http/request.nyx`)

The `HttpRequest` object contains:

| Property | Type | Description |
|----------|------|-------------|
| `method` | `string` | HTTP method (GET, POST, etc.) |
| `path` | `string` | Request path |
| `headers` | `Map<string, string>` | Request headers |
| `body` | `any` | Request body |
| `query_params` | `Map<string, string>` | Query parameters |
| `params` | `Map<string, string>` | Route parameters |

#### HTTP Methods Constant

```nyx
let GET = net.http.GET;
let POST = net.http.POST;
let PUT = net.http.PUT;
let DELETE = net.http.DELETE;
let PATCH = net.http.PATCH;
```

---

### HTTP Response (`http/response.nyx`)

Build HTTP responses with chained methods:

```nyx
// Basic responses
res.html("<h1>Hello</h1>");
res.json({"key": "value"});
res.text("Plain text");
res.status(404).html("<h1>Not Found</h1>");

// Set headers
res.headers["X-Custom"] = "value";

// Status codes
res.status(200);  // OK
res.status(201);  // Created
res.status(400);  // Bad Request
res.status(401);  // Unauthorized
res.status(403);  // Forbidden
res.status(404);  // Not Found
res.status(500);  // Internal Server Error
```

---

### Router (`http/router.nyx`)

Advanced routing with pattern matching:

```nyx
let router = net.http.Router();

// Add routes
router.get("/users", list_users);
router.post("/users", create_user);
router.get("/users/:id", get_user);

// Use with server
server.use_router(router);
```

---

### Middleware (`http/middleware/`)

#### Logger Middleware

Request logging with timing:

```nyx
import net.http.middleware;

// Add logger middleware
server.use(net.http.middleware.Logger());
```

#### CORS Middleware

```nyx
// Configure CORS
let cors = net.http.middleware.Cors({
    "origin": "*",
    "methods": ["GET", "POST", "PUT", "DELETE"],
    "headers": ["Content-Type"]
});

server.use(cors);
```

#### Rate Limiting

```nyx
let rate_limit = net.http.middleware.RateLimit({
    "max_requests": 100,
    "window_ms": 60000  // 1 minute
});

server.use(rate_limit);
```

---

### WebSocket Server (`websocket/`)

Real-time bidirectional communication:

```nyx
import net.websocket;

// Create WebSocket server
let ws = net.websocket.WebSocketServer(port: 8081);

ws.on_connect(fn(conn) {
    print("Client connected");
    
    // Send message to client
    conn.send("Welcome!");
    
    // Handle incoming messages
    conn.on_message(fn(msg) {
        print("Received: " + msg);
        // Broadcast to all clients
        ws.broadcast(msg);
    });
    
    // Handle disconnect
    conn.on_close(fn() {
        print("Client disconnected");
    });
});

ws.start();
```

#### Types

| Type | Description |
|------|-------------|
| [`websocket.WebSocketServer`](websocket/mod.nyx) | WebSocket server |
| [`websocket.WebSocketConnection`](websocket/server.nyx) | Client connection |
| [`websocket.WebSocketMessage`](websocket/server.nyx) | Message object |

---

### UDP Server (`udp/`)

Connectionless UDP datagrams:

```nyx
import net.udp;

// Create UDP server
let server = net.udp.Server(port: 8082);

server.on_packet(fn(data, addr) {
    print("Received from " + addr + ": " + data);
    
    // Respond
    server.send("Pong!", addr);
});

server.start();
```

---

### Static Files (`http/static.nyx`)

Serve static files with caching:

```nyx
import net.http.static;

// Create static handler
let handler = net.http.static.StaticHandler("./public", {
    "index_file": "index.html",
    "show_listing": false,
    "cache_max_age": 3600
});

// Mount on server
server.get("/*", fn(req, res) {
    let result = net.http.static.handle(handler, req);
    res.status(result.status_code);
    res.headers = result.headers;
    res.body = result.body;
});
```

---

### Templates (`templates/`)

Dynamic HTML rendering:

```nyx
import net.templates;

// Create template engine
let engine = net.templates.TemplateEngine(directory="./templates");

// Render template
let html = net.templates.render("index.html", {
    "title": "My Page",
    "name": "John",
    "items": ["Apple", "Banana", "Cherry"]
});

// Or render string directly
let output = net.templates.render_string("Hello {{ name }}!", {
    "name": "World"
});
```

Template syntax:
- `{{ variable }}` - Output variable
- `{% if condition %}` - Conditional
- `{% for item in items %}` - Loop

---

### JSON (`json/`)

JSON encoding and decoding:

```nyx
import net.json;

// Encode to JSON
let json_str = net.json.encode({
    "name": "John",
    "age": 30,
    "active": true
});

// Decode from JSON
let obj = net.json.decode('{"name": "John", "age": 30}');

// Pretty print
let pretty = net.json.pretty(data);

// Validate
if net.json.is_valid(json_str) {
    // Safe to decode
}
```

---

### Logging (`logging/`)

Structured logging:

```nyx
import net.logging;

// Create logger
let log = net.logging.Logger(name="my_app");

// Log at different levels
log.debug("Debug message");
log.info("Info message");
log.warn("Warning message");
log.error("Error message");

// Or use module-level functions
net.logging.info("Application started");
net.logging.error("Failed to connect");
```

#### Log Levels

```nyx
let DEBUG = net.logging.DEBUG;
let INFO = net.logging.INFO;
let WARN = net.logging.WARN;
let ERROR = net.logging.ERROR;
```

---

### Security (`security/`)

TLS/SSL support:

```nyx
import net.security;

// Configure TLS
let tls_config = {
    "cert_file": "./cert.pem",
    "key_file": "./key.pem",
    "enabled": true
};

// Create HTTPS server
let server = net.http.HttpServer(port: 443);
server.set_tls(tls_config);
server.start();
```

---

### Development Server (`dev_server.nyx`)

Hot reload and file watching:

```nyx
import net.dev_server;

// Create dev server
let dev = net.dev_server.DevServer(
    port: 3000,
    root: "./public",
    watch: ["./src", "./templates"],
    verbose: true
);

// Register change callback
dev.on_change(fn(file) {
    print("File changed: " + file);
});

// Start (blocks)
dev.start();

// Stop
dev.stop();
```

Features:
- File watching with auto-reload
- Live reload script injection
- Helpful error displays

---

## Usage Examples

### Basic HTTP Server

```nyx
import net.http;

fn main(args: List<string>) {
    let server = net.HttpServer(port: 8080);
    
    server.get("/", fn(req, res) {
        res.html("<h1>Hello from Nyx!</h1>");
    });
    
    server.start();
    print("Server running at http://localhost:8080");
}
```

### REST API

```nyx
import net.http;

// In-memory store
let users: List<Map<string, any>> = [];

fn main(args: List<string>) {
    let server = net.HttpServer(port: 8080);
    
    // List users
    server.get("/api/users", fn(req, res) {
        res.json({"data": users});
    });
    
    // Get user by ID
    server.get("/api/users/:id", fn(req, res) {
        let id = req.params["id"];
        // Find and return user...
    });
    
    // Create user
    server.post("/api/users", fn(req, res) {
        let new_user = req.body;
        users.push(new_user);
        res.status(201).json(new_user);
    });
    
    server.start();
}
```

### WebSocket Chat

```nyx
import net.http;
import net.websocket;

let clients: List<any> = [];

fn main(args: List<string>) {
    let server = net.HttpServer(port: 8080);
    
    // WebSocket endpoint
    server.get("/ws", fn(req, res) {
        let ws = net.websocket.connect("ws://mock");
        clients.push(ws);
        
        ws.on_message(fn(msg) {
            // Broadcast to all clients
            broadcast(msg);
        });
    });
    
    server.start();
}

fn broadcast(message: string) {
    for client in clients {
        client.send(message);
    }
}
```

### Static Files

```nyx
import net.http;
import net.http.static;

fn main(args: List<string>) {
    let server = net.HttpServer(port: 8080);
    
    let handler = net.http.static.StaticHandler("./public", {
        "index_file": "index.html",
        "cache_max_age": 3600
    });
    
    server.get("/*", fn(req, res) {
        let result = net.http.static.handle(handler, req);
        res.status(result.status_code);
        res.body = result.body;
    });
    
    server.start();
}
```

### Templates

```nyx
import net.http;
import net.templates;

fn main(args: List<string>) {
    let server = net.HttpServer(port: 8080);
    
    server.get("/", fn(req, res) {
        let html = net.templates.render("home.html", {
            "title": "Welcome",
            "user": "John",
            "posts": [
                {"title": "First Post"},
                {"title": "Second Post"}
            ]
        });
        res.html(html);
    });
    
    server.start();
}
```

---

## CLI Commands

### `nyx serve`

Start a development server with hot reload:

```bash
nyx serve                    # Default port 3000
nyx serve --port 8080       # Custom port
nyx serve --root ./public   # Static files directory
nyx serve --watch ./src     # Watch directories
```

### `nyx dev`

Start development server with file watching:

```bash
nyx dev                     # Start with defaults
nyx dev --port 3000        # Custom port
nyx dev --verbose          # Verbose output
```

### `nyx start`

Start a production server:

```bash
nyx start                   # Start server defined in project
nyx start --port 8080       # Override port
nyx start --host 0.0.0.0   # Override host
```

---

## Configuration

### Server Configuration

```nyx
let server = net.HttpServer(
    host: "0.0.0.0",  // Bind address
    port: 8080        // Port number
);
```

### TLS/SSL Configuration

```nyx
let config = {
    "cert_file": "/path/to/cert.pem",
    "key_file": "/path/to/key.pem",
    "enabled": true,
    "protocols": ["TLSv1.2", "TLSv1.3"]
};
```

### Middleware Configuration

```nyx
// Rate limiting
server.use(net.http.middleware.RateLimit({
    "max_requests": 100,
    "window_ms": 60000
}));

// CORS
server.use(net.http.middleware.Cors({
    "origin": "https://example.com",
    "methods": ["GET", "POST"],
    "credentials": true
}));

// Logger
server.use(net.http.middleware.Logger());
```

### Static Files Configuration

```nyx
let handler = net.http.static.StaticHandler("./public", {
    "index_file": "index.html",
    "show_listing": true,
    "cache_max_age": 3600,
    "max_file_size": 10485760  // 10MB
});
```

---

## Best Practices

### Production Deployment

1. **Use reverse proxy** (nginx, Apache) for:
   - Load balancing
   - SSL termination
   - Static file serving
   - Caching

2. **Configure proper timeouts**:
   ```nyx
   server.set_timeout(30000);  // 30 second request timeout
   ```

3. **Enable keep-alive** for connection reuse

4. **Use gzip compression**:
   ```nyx
   server.use(net.http.middleware.Compression());
   ```

### Security Considerations

1. **Always use TLS in production**
   ```nyx
   server.set_tls({
       "cert_file": "./cert.pem",
       "key_file": "./key.pem"
   });
   ```

2. **Validate all input**
   ```nyx
   server.post("/api/users", fn(req, res) {
       if not validate_input(req.body) {
           res.status(400).json({"error": "Invalid input"});
           return;
       }
       // Process request
   });
   ```

3. **Implement rate limiting** to prevent abuse

4. **Use CORS middleware** properly for API access

5. **Don't expose sensitive data** in errors

### Performance Optimization

1. **Connection pooling** for database access

2. **Cache expensive operations**:
   ```nyx
   // Use caching middleware
   server.use(net.http.middleware.Cache({
       "max_age": 3600,
       "cache_control": "public"
   }));
   ```

3. **Use async I/O** throughout

4. **Limit request body sizes**:
   ```nyx
   server.set_max_body_size(1048576);  // 1MB
   ```

5. **Monitor with logging**:
   ```nyx
   server.use(net.http.middleware.Logger());
   ```

---

## Examples

See the [`examples/server/`](../examples/server/) directory for complete examples:

| Example | Description |
|---------|-------------|
| [`hello_world.nyx`](../examples/server/hello_world.nyx) | Basic HTTP server |
| [`api_server.nyx`](../examples/server/api_server.nyx) | RESTful API |
| [`websocket_chat.nyx`](../examples/server/websocket_chat.nyx) | Real-time chat |
| [`static_files.nyx`](../examples/server/static_files.nyx) | Static file serving |
| [`template_app.nyx`](../examples/server/template_app.nyx) | Dynamic templates |

---

## License

See Nyx project license for details.
