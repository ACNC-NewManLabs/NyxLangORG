#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#  define WIN32_LEAN_AND_MEAN
#  include <winsock2.h>
#  include <ws2tcpip.h>
#  pragma comment(lib, "ws2_32.lib")
typedef SOCKET nyx_socket_t;
#  define nyx_close closesocket
#else
#  include <errno.h>
#  include <netinet/in.h>
#  include <sys/socket.h>
#  include <unistd.h>
typedef int nyx_socket_t;
#  define nyx_close close
#endif

static int nyx_http_listen(uint16_t port);
static void nyx_send_response(nyx_socket_t client, const char* body);
static char* nyx_load_entry_file(void);

// These exported symbols match what the current LLVM backend emits.
// They are intentionally minimal: enough to link and run a basic dev server.

int64_t nyx_ui_div(int64_t props, int64_t children) __asm__("ui::div");
int64_t nyx_ui_footer(int64_t props, int64_t children) __asm__("ui::footer");

int64_t nyx_router_new_router(void) __asm__("router::new_router");
int64_t nyx_router_get(int64_t router, int64_t path, int64_t handler) __asm__("router::get");

int64_t nyx_server_default_config(void) __asm__("server::default_config");
int64_t nyx_server_new_server(int64_t config) __asm__("server::new_server");
int64_t nyx_server_use_router(int64_t server, int64_t router) __asm__("server::use_router");
int64_t nyx_server_listen(int64_t server, int64_t host, int64_t port) __asm__("server::listen");

int64_t nyx_console_log(int64_t msg) __asm__("console::log");

int64_t nyx_ui_div(int64_t props, int64_t children) {
  (void)props;
  (void)children;
  return 0;
}

int64_t nyx_ui_footer(int64_t props, int64_t children) {
  (void)props;
  (void)children;
  return 0;
}

int64_t nyx_router_new_router(void) { return 1; }
int64_t nyx_router_get(int64_t router, int64_t path, int64_t handler) {
  (void)router;
  (void)path;
  (void)handler;
  return 0;
}

int64_t nyx_server_default_config(void) { return 1; }
int64_t nyx_server_new_server(int64_t config) {
  (void)config;
  return 1;
}
int64_t nyx_server_use_router(int64_t server, int64_t router) {
  (void)server;
  (void)router;
  return 0;
}

int64_t nyx_console_log(int64_t msg) {
  (void)msg;
  // Current lowering passes integers only; keep output minimal.
  return 0;
}

int64_t nyx_server_listen(int64_t server, int64_t host, int64_t port) {
  (void)server;
  (void)host;

  uint16_t p = 8000;
  if (port > 0 && port < 65536) {
    p = (uint16_t)port;
  }

  fprintf(stdout, "Nyx Website starting at http://localhost:%u\n", (unsigned)p);
  fflush(stdout);

  return nyx_http_listen(p);
}

static int nyx_http_listen(uint16_t port) {
#ifdef _WIN32
  WSADATA wsa;
  if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
    fprintf(stderr, "NYX runtime: WSAStartup failed\n");
    return -1;
  }
#endif

  nyx_socket_t sock = (nyx_socket_t)socket(AF_INET, SOCK_STREAM, 0);
#ifdef _WIN32
  if (sock == INVALID_SOCKET) {
    fprintf(stderr, "NYX runtime: socket() failed\n");
    return -1;
  }
#else
  if (sock < 0) {
    fprintf(stderr, "NYX runtime: socket() failed: %s\n", strerror(errno));
    return -1;
  }
#endif

  int yes = 1;
#ifdef _WIN32
  setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, (const char*)&yes, (int)sizeof(yes));
#else
  setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &yes, (socklen_t)sizeof(yes));
#endif

  struct sockaddr_in addr;
  memset(&addr, 0, sizeof(addr));
  addr.sin_family = AF_INET;
  addr.sin_addr.s_addr = htonl(INADDR_ANY);
  addr.sin_port = htons(port);

#ifdef _WIN32
  if (bind(sock, (struct sockaddr*)&addr, (int)sizeof(addr)) != 0) {
    fprintf(stderr, "NYX runtime: bind() failed\n");
    nyx_close(sock);
    return -1;
  }
  if (listen(sock, 16) != 0) {
    fprintf(stderr, "NYX runtime: listen() failed\n");
    nyx_close(sock);
    return -1;
  }
#else
  if (bind(sock, (struct sockaddr*)&addr, sizeof(addr)) != 0) {
    fprintf(stderr, "NYX runtime: bind() failed: %s\n", strerror(errno));
    nyx_close(sock);
    return -1;
  }
  if (listen(sock, 16) != 0) {
    fprintf(stderr, "NYX runtime: listen() failed: %s\n", strerror(errno));
    nyx_close(sock);
    return -1;
  }
#endif

  for (;;) {
#ifdef _WIN32
    nyx_socket_t client = accept(sock, NULL, NULL);
    if (client == INVALID_SOCKET) {
      continue;
    }
#else
    nyx_socket_t client = accept(sock, NULL, NULL);
    if (client < 0) {
      continue;
    }
#endif

    char* body = nyx_load_entry_file();
    nyx_send_response(client, body);
    free(body);
    nyx_close(client);
  }

  // unreachable
  // nyx_close(sock);
  // return 0;
}

static void nyx_send_response(nyx_socket_t client, const char* body) {
  const char* header =
      "HTTP/1.1 200 OK\r\n"
      "Content-Type: text/html; charset=utf-8\r\n"
      "Cache-Control: no-cache\r\n"
      "Connection: close\r\n\r\n";

#ifdef _WIN32
  send(client, header, (int)strlen(header), 0);
  send(client, body, (int)strlen(body), 0);
#else
  (void)write(client, header, strlen(header));
  (void)write(client, body, strlen(body));
#endif
}

static char* nyx_load_entry_file(void) {
  const char* path = getenv("NYX_ENTRY_FILE");
  if (path == NULL || path[0] == '\0') {
    const char* fallback =
        "<html><head><title>Nyx</title></head><body>"
        "<h1>Nyx Dev Server</h1>"
        "<p>Set <code>NYX_ENTRY_FILE</code> to render the entry file content.</p>"
        "</body></html>";
    char* out = (char*)malloc(strlen(fallback) + 1);
    strcpy(out, fallback);
    return out;
  }

  FILE* f = fopen(path, "rb");
  if (!f) {
    const char* msg_prefix =
        "<html><head><title>Nyx</title></head><body>"
        "<h1>Nyx Dev Server</h1><pre>Could not open entry file: ";
    const char* msg_suffix = "</pre></body></html>";
    size_t n = strlen(msg_prefix) + strlen(path) + strlen(msg_suffix) + 1;
    char* out = (char*)malloc(n);
    snprintf(out, n, "%s%s%s", msg_prefix, path, msg_suffix);
    return out;
  }

  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);

  if (size < 0) size = 0;
  char* raw = (char*)malloc((size_t)size + 1);
  size_t read_n = fread(raw, 1, (size_t)size, f);
  raw[read_n] = '\0';
  fclose(f);

  // Escape HTML.
  size_t cap = read_n * 6 + 512;
  char* esc = (char*)malloc(cap);
  size_t j = 0;
  for (size_t i = 0; i < read_n; i++) {
    const char c = raw[i];
    const char* rep = NULL;
    switch (c) {
      case '&': rep = "&amp;"; break;
      case '<': rep = "&lt;"; break;
      case '>': rep = "&gt;"; break;
      case '"': rep = "&quot;"; break;
      default: break;
    }
    if (rep) {
      size_t rlen = strlen(rep);
      memcpy(esc + j, rep, rlen);
      j += rlen;
    } else {
      esc[j++] = c;
    }
    if (j + 16 > cap) {
      cap *= 2;
      esc = (char*)realloc(esc, cap);
    }
  }
  esc[j] = '\0';
  free(raw);

  const char* prefix =
      "<html><head><title>Nyx</title>"
      "<style>body{font-family:ui-monospace,Menlo,monospace;background:#0b0b0b;color:#eee;padding:24px}"
      "pre{white-space:pre-wrap;word-break:break-word;background:#111;padding:16px;border-radius:8px;border:1px solid #222}"
      "h1{margin:0 0 12px 0}</style>"
      "</head><body><h1>Nyx Dev Server (runtime stubs)</h1>"
      "<p>This is a placeholder renderer. UI/web engine runtime is not yet linked into the LLVM backend.</p>"
      "<pre>";
  const char* suffix = "</pre></body></html>";

  size_t total = strlen(prefix) + strlen(esc) + strlen(suffix) + 1;
  char* out = (char*)malloc(total);
  snprintf(out, total, "%s%s%s", prefix, esc, suffix);
  free(esc);
  return out;
}

