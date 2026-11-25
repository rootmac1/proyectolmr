// src/trade_bridge.c
// Puente nativo de Red, WebSockets, Multithreading y Motor Gráfico 2D para LMR

#ifdef _WIN32
#define _CRT_SECURE_NO_WARNINGS
#endif
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <time.h>
#include <limits.h>
#include <stdint.h>
#include <errno.h>
#include <math.h>
#ifdef _WIN32
#include <io.h>
#include <windows.h>
#include <shellapi.h>
#define LMR_STRNICMP _strnicmp
#define POPEN _popen
#define PCLOSE _pclose
#else
#include <pthread.h>
#include <strings.h>
#include <unistd.h>
#define LMR_STRNICMP strncasecmp
#define POPEN popen
#define PCLOSE pclose
#endif

// Checks shared by LLVM code generated for collection indexing.
void lmr_check_array_index(long long index, long long length) {
  if (index < 0 || index >= length) {
    fprintf(stderr, "LMR runtime error: índice %lld fuera de rango (longitud %lld)\n",
            index, length);
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
}

void lmr_check_array_size(long long size) {
  if (size < 0) {
    fprintf(stderr, "LMR runtime error: tamaño de array negativo (%lld)\n", size);
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
}

long long lmr_checked_array_bytes(long long size) {
  lmr_check_array_size(size);
  if (size > LLONG_MAX / 8) {
    fprintf(stderr, "LMR runtime error: tamaño de array demasiado grande\n");
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
  return size * 8;
}

typedef struct LmrAllocation {
  void *pointer;
  size_t size;
  void (*finalizer)(void *);
  int marked;
  struct LmrAllocation *next;
} LmrAllocation;

typedef struct LmrRoot {
  void **slot;
  struct LmrRoot *next;
} LmrRoot;

static LmrAllocation *lmr_allocations = NULL;
static LmrRoot *lmr_roots = NULL;
static int lmr_cleanup_registered = 0;
static int lmr_gc_disabled = 0;
static int lmr_finalizing = 0;
static int lmr_function_depth = 0;
#ifdef _WIN32
static SRWLOCK lmr_allocations_lock = SRWLOCK_INIT;
#define LMR_LOCK_ALLOCATIONS() AcquireSRWLockExclusive(&lmr_allocations_lock)
#define LMR_UNLOCK_ALLOCATIONS() ReleaseSRWLockExclusive(&lmr_allocations_lock)
#else
static pthread_mutex_t lmr_allocations_lock = PTHREAD_MUTEX_INITIALIZER;
#define LMR_LOCK_ALLOCATIONS() pthread_mutex_lock(&lmr_allocations_lock)
#define LMR_UNLOCK_ALLOCATIONS() pthread_mutex_unlock(&lmr_allocations_lock)
#endif

static void lmr_release_allocations(void) {
  LmrAllocation *release_list = NULL;
  LmrRoot *root = NULL;
  int first_pass = 1;
  LMR_LOCK_ALLOCATIONS();
  lmr_finalizing = 1;
  LMR_UNLOCK_ALLOCATIONS();
  for (;;) {
    LMR_LOCK_ALLOCATIONS();
    LmrAllocation *batch = lmr_allocations;
    lmr_allocations = NULL;
    if (first_pass) { root = lmr_roots; lmr_roots = NULL; first_pass = 0; }
    LMR_UNLOCK_ALLOCATIONS();
    if (!batch) break;
    for (LmrAllocation *item = batch; item; item = item->next) {
      void (*finalizer)(void *) = item->finalizer;
      item->finalizer = NULL;
      if (finalizer) finalizer(item->pointer);
    }
    LmrAllocation *tail = batch;
    while (tail->next) tail = tail->next;
    tail->next = release_list;
    release_list = batch;
  }
  while (release_list) {
    LmrAllocation *next = release_list->next;
    free(release_list->pointer);
    free(release_list);
    release_list = next;
  }
  LMR_LOCK_ALLOCATIONS();
  lmr_finalizing = 0;
  LMR_UNLOCK_ALLOCATIONS();
  while (root) {
    LmrRoot *next = root->next;
    free(root);
    root = next;
  }
}

static void lmr_allocation_failure(const char *message) {
  fprintf(stderr, "LMR runtime error: %s\n", message);
  fflush(stderr);
  exit(EXIT_FAILURE);
}

void *lmr_malloc(long long size) {
  if (size < 0) {
    lmr_allocation_failure("solicitud de memoria con tamaño negativo");
  }
  if ((unsigned long long)size > (unsigned long long)SIZE_MAX)
    lmr_allocation_failure("tamaño de memoria fuera del límite de la plataforma");
  size_t actual_size = size == 0 ? 1u : (size_t)size;
  void *result = malloc(actual_size);
  if (result == NULL)
    lmr_allocation_failure("no se pudo reservar memoria");
  LmrAllocation *allocation = (LmrAllocation *)malloc(sizeof(LmrAllocation));
  if (allocation == NULL) {
    free(result);
    lmr_allocation_failure("no se pudo registrar una reserva de memoria");
  }
  allocation->pointer = result;
  allocation->size = actual_size;
  allocation->finalizer = NULL;
  allocation->marked = 0;
  LMR_LOCK_ALLOCATIONS();
  allocation->next = lmr_allocations;
  lmr_allocations = allocation;
  int register_cleanup = !lmr_cleanup_registered;
  lmr_cleanup_registered = 1;
  LMR_UNLOCK_ALLOCATIONS();
  if (register_cleanup) (void)atexit(lmr_release_allocations);
  return result;
}

void lmr_register_root(void *slot) {
  if (!slot) return;
  LmrRoot *root = (LmrRoot *)malloc(sizeof(LmrRoot));
  if (!root) lmr_allocation_failure("no se pudo registrar una raíz de memoria");
  root->slot = (void **)slot;
  LMR_LOCK_ALLOCATIONS();
  root->next = lmr_roots;
  lmr_roots = root;
  LMR_UNLOCK_ALLOCATIONS();
}

void lmr_unregister_root(void *slot) {
  if (!slot) return;
  LMR_LOCK_ALLOCATIONS();
  LmrRoot **link = &lmr_roots;
  while (*link && (*link)->slot != (void **)slot) link = &(*link)->next;
  LmrRoot *root = *link;
  if (root) *link = root->next;
  LMR_UNLOCK_ALLOCATIONS();
  free(root);
}

void lmr_register_finalizer(void *pointer, void *function_pointer) {
  if (!pointer || !function_pointer) return;
  LMR_LOCK_ALLOCATIONS();
  for (LmrAllocation *allocation = lmr_allocations; allocation; allocation = allocation->next) {
    if (allocation->pointer == pointer) {
      allocation->finalizer = (void (*)(void *))function_pointer;
      break;
    }
  }
  LMR_UNLOCK_ALLOCATIONS();
}

static int lmr_mark_pointer(void *candidate) {
  if (!candidate) return 0;
  for (LmrAllocation *allocation = lmr_allocations; allocation; allocation = allocation->next) {
    if (allocation->pointer == candidate && !allocation->marked) {
      allocation->marked = 1;
      return 1;
    }
  }
  return 0;
}

void lmr_collect(void) {
  LMR_LOCK_ALLOCATIONS();
  if (lmr_gc_disabled || lmr_finalizing || lmr_function_depth > 1) { LMR_UNLOCK_ALLOCATIONS(); return; }
  for (LmrAllocation *allocation = lmr_allocations; allocation; allocation = allocation->next)
    allocation->marked = 0;
  for (LmrRoot *root = lmr_roots; root; root = root->next) {
    void *candidate = NULL;
    if (root->slot) memcpy(&candidate, root->slot, sizeof(candidate));
    (void)lmr_mark_pointer(candidate);
  }

  int changed;
  do {
    changed = 0;
    for (LmrAllocation *allocation = lmr_allocations; allocation; allocation = allocation->next) {
      if (!allocation->marked) continue;
      const unsigned char *bytes = (const unsigned char *)allocation->pointer;
      for (size_t offset = 0; offset + sizeof(void *) <= allocation->size; offset += sizeof(void *)) {
        void *candidate = NULL;
        memcpy(&candidate, bytes + offset, sizeof(candidate));
        changed |= lmr_mark_pointer(candidate);
      }
    }
  } while (changed);

  LmrAllocation *garbage = NULL;
  LmrAllocation **link = &lmr_allocations;
  while (*link) {
    LmrAllocation *allocation = *link;
    if (allocation->marked) {
      link = &allocation->next;
    } else {
      *link = allocation->next;
      allocation->next = garbage;
      garbage = allocation;
    }
  }
  if (garbage) lmr_finalizing = 1;
  LMR_UNLOCK_ALLOCATIONS();

  while (garbage) {
    LmrAllocation *next = garbage->next;
    if (garbage->finalizer) garbage->finalizer(garbage->pointer);
    free(garbage->pointer);
    free(garbage);
    garbage = next;
  }
  LMR_LOCK_ALLOCATIONS();
  lmr_finalizing = 0;
  LMR_UNLOCK_ALLOCATIONS();
}

void lmr_enter_function(void) {
  LMR_LOCK_ALLOCATIONS();
  lmr_function_depth++;
  LMR_UNLOCK_ALLOCATIONS();
}

void lmr_leave_function(void) {
  LMR_LOCK_ALLOCATIONS();
  if (lmr_function_depth > 0) lmr_function_depth--;
  LMR_UNLOCK_ALLOCATIONS();
}

void lmr_disable_gc(void) {
  LMR_LOCK_ALLOCATIONS();
  lmr_gc_disabled = 1;
  LMR_UNLOCK_ALLOCATIONS();
}

void lmr_free(void *pointer) {
  // This is a compatibility hint only. The tracing collector owns physical
  // release so one scope cannot invalidate another alias. Unknown pointers
  // from external C APIs are borrowed and are not freed here.
  (void)pointer;
}

void *lmr_realloc(void *pointer, long long size) {
  if (pointer == NULL) return lmr_malloc(size);
  if (size < 0 || (unsigned long long)size > (unsigned long long)SIZE_MAX)
    lmr_allocation_failure("tamaño inválido al redimensionar memoria");
  size_t actual_size = size == 0 ? 1u : (size_t)size;
  LMR_LOCK_ALLOCATIONS();
  LmrAllocation *allocation = lmr_allocations;
  while (allocation && allocation->pointer != pointer) allocation = allocation->next;
  void *resized = realloc(pointer, actual_size);
  if (resized && allocation) {
    allocation->pointer = resized;
    allocation->size = actual_size;
  }
  LMR_UNLOCK_ALLOCATIONS();
  if (!resized) lmr_allocation_failure("no se pudo redimensionar memoria");
  if (!allocation) {
    LmrAllocation *new_allocation = (LmrAllocation *)malloc(sizeof(LmrAllocation));
    if (!new_allocation) {
      free(resized);
      lmr_allocation_failure("no se pudo registrar memoria redimensionada");
    }
    new_allocation->pointer = resized;
    new_allocation->size = actual_size;
    new_allocation->finalizer = NULL;
    new_allocation->marked = 0;
    LMR_LOCK_ALLOCATIONS();
    new_allocation->next = lmr_allocations;
    lmr_allocations = new_allocation;
    int register_cleanup = !lmr_cleanup_registered;
    lmr_cleanup_registered = 1;
    LMR_UNLOCK_ALLOCATIONS();
    if (register_cleanup) (void)atexit(lmr_release_allocations);
  }
  return resized;
}

// Route the bridge's own transient buffers through the same registry. Keep
// the allocator implementations above these macros on the C runtime allocator.
#define malloc(size) lmr_malloc((long long)(size))
#define free(pointer) lmr_free((pointer))
#define realloc(pointer, size) lmr_realloc((pointer), (long long)(size))

void lmr_check_integer_division(long long dividend, long long divisor) {
  if (divisor == 0) {
    fprintf(stderr, "LMR runtime error: división entera entre cero\n");
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
  if (dividend == LLONG_MIN && divisor == -1) {
    fprintf(stderr, "LMR runtime error: desbordamiento en división entera\n");
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
}

void lmr_check_float_division(double divisor) {
  if (divisor == 0.0) {
    fprintf(stderr, "LMR runtime error: división de punto flotante entre cero\n");
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
}

void lmr_check_map_capacity(long long size) {
  if (size >= 100) {
    fprintf(stderr, "LMR runtime error: capacidad máxima del map (100) excedida\n");
    fflush(stderr);
    exit(EXIT_FAILURE);
  }
}

void lmr_missing_map_key(void) {
  fprintf(stderr, "LMR runtime error: clave inexistente en map\n");
  fflush(stderr);
  exit(EXIT_FAILURE);
}

// ============================================================
// 1. PETICIÓN HTTP GET (REST API)
// ============================================================
static int lmr_curl_url_is_safe(const char *url, int websocket) {
  if (!url)
    return 0;
  size_t length = strlen(url);
  if (length == 0 || length > 1800)
    return 0;

  const char *authority = NULL;
  if (websocket) {
    if (strncmp(url, "ws://", 5) == 0)
      authority = url + 5;
    else if (strncmp(url, "wss://", 6) == 0)
      authority = url + 6;
    else
      return 0;
  } else {
    if (strncmp(url, "http://", 7) == 0)
      authority = url + 7;
    else if (strncmp(url, "https://", 8) == 0)
      authority = url + 8;
    else
      return 0;
  }

  const char *authority_end = strpbrk(authority, "/?#");
  if (!authority_end)
    authority_end = url + length;
  if (authority == authority_end || memchr(authority, '@', (size_t)(authority_end - authority)))
    return 0;

  // The URL is inserted into a quoted command for popen/_popen. Reject shell
  // expansions, quote breaks, control chars and cmd.exe percent expansion.
  const char *forbidden = "\"\\$`%!<>|^";
  for (const unsigned char *p = (const unsigned char *)url; *p; ++p) {
    if (*p <= 0x20 || *p >= 0x7f || strchr(forbidden, *p))
      return 0;
  }
  return 1;
}

static char *lmr_empty_string(void) {
  char *empty = (char *)malloc(1);
  if (empty)
    empty[0] = '\0';
  return empty;
}

char *lmr_http_get(const char *url) {
  if (!lmr_curl_url_is_safe(url, 0))
    return lmr_empty_string();

  char cmd[2048];
  int cmd_len = snprintf(cmd, sizeof(cmd),
                         "curl --proto =http,https --proto-redir =http,https -s -L --retry 2 --retry-delay 1 --connect-timeout 10 --max-time 30 -- \"%s\"",
                         url);
  if (cmd_len < 0 || (size_t)cmd_len >= sizeof(cmd))
    return lmr_empty_string();

  FILE *fp = POPEN(cmd, "r");
  if (!fp) {
    return lmr_empty_string();
  }

  size_t cap = 4096;
  size_t len = 0;
  char *buf = (char *)malloc(cap);
  if (!buf) {
    PCLOSE(fp);
    return lmr_empty_string();
  }

  char temp[1024];
  while (fgets(temp, sizeof(temp), fp)) {
    size_t chunk = strlen(temp);
    if (chunk > SIZE_MAX - len - 1) {
      free(buf);
      PCLOSE(fp);
      return lmr_empty_string();
    }
    size_t needed = len + chunk + 1;
    if (needed > cap) {
      size_t new_cap = cap;
      while (new_cap < needed && new_cap <= SIZE_MAX / 2)
        new_cap *= 2;
      if (new_cap < needed) {
        free(buf);
        PCLOSE(fp);
        return lmr_empty_string();
      }
      char *grown = (char *)realloc(buf, new_cap);
      if (!grown) {
        free(buf);
        PCLOSE(fp);
        return lmr_empty_string();
      }
      buf = grown;
      cap = new_cap;
    }
    memcpy(buf + len, temp, chunk);
    len += chunk;
  }
  buf[len] = '\0';
  PCLOSE(fp);
  return buf;
}

// ============================================================
// 2. CLIENTE WEBSOCKET (STREAM EN TIEMPO REAL)
// ============================================================
typedef struct {
  FILE *fp;
} LmrWebSocket;

void *lmr_ws_connect(const char *wss_url) {
  if (!lmr_curl_url_is_safe(wss_url, 1))
    return NULL;

  char cmd[2048];
  int cmd_len = snprintf(cmd, sizeof(cmd),
                         "curl -s -N -i --no-buffer --http1.1 -H \"Upgrade: websocket\" -H "
                         "\"Connection: Upgrade\" -H \"Sec-WebSocket-Key: "
                         "SGVsbG8sIHdvcmxkIQ==\" -H \"Sec-WebSocket-Version: 13\" -- \"%s\"",
                         wss_url);
  if (cmd_len < 0 || (size_t)cmd_len >= sizeof(cmd))
    return NULL;

  FILE *fp = POPEN(cmd, "r");
  if (!fp)
    return NULL;

  LmrWebSocket *ws = (LmrWebSocket *)malloc(sizeof(LmrWebSocket));
  if (!ws) {
    PCLOSE(fp);
    return NULL;
  }
  ws->fp = fp;
  return (void *)ws;
}

char *lmr_ws_read(void *handle) {
  if (!handle)
    return lmr_empty_string();

  LmrWebSocket *ws = (LmrWebSocket *)handle;
  char temp[4096];

  if (fgets(temp, sizeof(temp), ws->fp)) {
    size_t len = strlen(temp);
    char *res = (char *)malloc(len + 1);
    if (!res)
      return lmr_empty_string();
    memcpy(res, temp, len);
    res[len] = '\0';
    return res;
  }

  return lmr_empty_string();
}

void lmr_ws_close(void *handle) {
  if (!handle)
    return;
  LmrWebSocket *ws = (LmrWebSocket *)handle;
  if (ws->fp) {
    PCLOSE(ws->fp);
  }
  free(ws);
}

// ============================================================
// 3. EXTRACTORES Y PARSER JSON COMPLETO
// ============================================================
double lmr_json_get_number(const char *json, const char *key) {
  char pattern[256];
  snprintf(pattern, sizeof(pattern), "\"%s\":", key);

  char *pos = strstr(json, pattern);
  if (!pos) {
    snprintf(pattern, sizeof(pattern), "\"%s\":\"", key);
    pos = strstr(json, pattern);
    if (!pos)
      return 0.0;
    pos += strlen(pattern);
    return atof(pos);
  }

  pos += strlen(pattern);
  while (*pos == ' ' || *pos == '\"')
    pos++;
  return atof(pos);
}

typedef struct {
  long long size;
  char **keys;
  char **values;
} LmrMap;

void *lmr_json_parse(const char *json) {
  LmrMap *m = (LmrMap *)malloc(sizeof(LmrMap));
  m->size = 0;
  m->keys = (char **)malloc(sizeof(char *) * 200);
  m->values = (char **)malloc(sizeof(char *) * 200);

  const char *p = json;
  while (*p) {
    if (*p == '\"') {
      p++;
      const char *k_start = p;
      while (*p && *p != '\"')
        p++;
      if (!*p)
        break;
      size_t k_len = p - k_start;
      p++;

      while (*p && (*p == ' ' || *p == ':' || *p == '\t' || *p == '\r' ||
                    *p == '\n')) {
        if (*p == ':') {
          p++;
          break;
        }
        p++;
      }
      while (*p && (*p == ' ' || *p == '\t'))
        p++;

      char val_buf[1024];
      size_t v_len = 0;

      if (*p == '\"') {
        p++;
        const char *v_start = p;
        while (*p && *p != '\"')
          p++;
        v_len = p - v_start;
        if (v_len > 1023)
          v_len = 1023;
        memcpy(val_buf, v_start, v_len);
        if (*p == '\"')
          p++;
      } else {
        const char *v_start = p;
        while (*p && *p != ',' && *p != '}' && *p != ']' && *p != ' ' &&
               *p != '\n')
          p++;
        v_len = p - v_start;
        if (v_len > 1023)
          v_len = 1023;
        memcpy(val_buf, v_start, v_len);
      }
      val_buf[v_len] = '\0';

      char *k_copy = (char *)malloc(k_len + 1);
      memcpy(k_copy, k_start, k_len);
      k_copy[k_len] = '\0';

      char *v_copy = (char *)malloc(v_len + 1);
      memcpy(v_copy, val_buf, v_len);
      v_copy[v_len] = '\0';

      m->keys[m->size] = k_copy;
      m->values[m->size] = v_copy;
      m->size++;
      if (m->size >= 200)
        break;
    } else {
      p++;
    }
  }

  return (void *)m;
}

// ============================================================
// 4. TEMPORIZADOR Y PAUSA NATIVA (SLEEP)
// ============================================================
void lmr_sleep(int ms) {
#ifdef _WIN32
  Sleep(ms);
#else
  usleep(ms * 1000);
#endif
}

// ============================================================
// 5. LANZAMIENTO DE HILOS ASÍNCRONOS (SPAWN MULTITHREADING)
// ============================================================
#ifdef _WIN32
static DWORD WINAPI lmr_win_thread_runner(LPVOID param) {
  void (*fn)(void) = (void (*)(void))param;
  if (fn)
    fn();
  return 0;
}

void lmr_spawn_thread(void *fn_ptr) {
  lmr_disable_gc();
  CreateThread(NULL, 0, lmr_win_thread_runner, fn_ptr, 0, NULL);
}
#else
static void *lmr_posix_thread_runner(void *param) {
  void (*fn)(void) = (void (*)(void))param;
  if (fn)
    fn();
  return NULL;
}

void lmr_spawn_thread(void *fn_ptr) {
  lmr_disable_gc();
  pthread_t t;
  pthread_create(&t, NULL, lmr_posix_thread_runner, fn_ptr);
  pthread_detach(t);
}
#endif

// ============================================================
// 6. MOTOR GRÁFICO 2D EN VENTANA NATIVA (ESTILO RAYLIB)
// ============================================================
#ifdef _WIN32
static HWND g_hwnd = NULL;
static HDC g_hdc_mem = NULL;
static HBITMAP g_hbm_mem = NULL;
static int g_win_width = 800;
static int g_win_height = 600;
static int g_should_close = 0;
static int g_mouse_wheel_delta = 0;
static int g_scroll_offset = 0;
static RECT g_scroll_viewport = {0, 0, 0, 0};
static int g_mouse_click_x = -1;
static int g_mouse_click_y = -1;
static int g_mouse_clicked = 0;
static int g_report_expanded[2048];
static int g_report_filter = 0;
static int g_report_search_focus = 0;
static int g_report_keyboard_focus = 1;
static int g_report_selected_row = 0;
static int g_report_selected_col = 0;
static int g_report_key = 0;
static int g_report_keep_selection_visible = 0;
static char g_report_search[128] = {0};
static int g_report_search_len = 0;

static LRESULT CALLBACK lmr_wnd_proc(HWND hwnd, UINT msg, WPARAM wParam,
                                     LPARAM lParam) {
  switch (msg) {
  case WM_CLOSE:
  case WM_DESTROY:
    g_should_close = 1;
    PostQuitMessage(0);
    return 0;
  case WM_MOUSEWHEEL: {
    POINT point = {(short)LOWORD(lParam), (short)HIWORD(lParam)};
    ScreenToClient(hwnd, &point);
    if (point.x >= g_scroll_viewport.left &&
        point.x < g_scroll_viewport.right &&
        point.y >= g_scroll_viewport.top && point.y < g_scroll_viewport.bottom)
      g_mouse_wheel_delta += GET_WHEEL_DELTA_WPARAM(wParam);
    return 0;
  }
  case WM_LBUTTONDOWN:
    g_mouse_click_x = (short)LOWORD(lParam);
    g_mouse_click_y = (short)HIWORD(lParam);
    g_mouse_clicked = 1;
    SetFocus(hwnd);
    SetCapture(hwnd);
    return 0;
  case WM_LBUTTONUP:
    ReleaseCapture();
    return 0;
  case WM_SETFOCUS:
    if (!g_report_search_focus) g_report_keyboard_focus = 1;
    return 0;
  case WM_KILLFOCUS:
    g_report_keyboard_focus = 0;
    return 0;
  case WM_CHAR:
    if (g_report_search_focus) {
      if (wParam == 13) {
        g_report_search_focus = 0;
        g_report_keyboard_focus = 1;
        return 0;
      }
      if (wParam == 8 && g_report_search_len > 0) {
        g_report_search[--g_report_search_len] = '\0';
      } else if (wParam >= 32 && wParam < 127 && g_report_search_len < 126) {
        g_report_search[g_report_search_len++] = (char)wParam;
        g_report_search[g_report_search_len] = '\0';
      }
      return 0;
    }
    break;
  case WM_KEYDOWN:
    if (wParam == VK_ESCAPE && g_report_search_focus) {
      g_report_search_focus = 0;
      g_report_keyboard_focus = 1;
      return 0;
    }
    if (!g_report_search_focus && g_report_keyboard_focus &&
        (wParam == VK_UP || wParam == VK_DOWN || wParam == VK_LEFT ||
         wParam == VK_RIGHT || wParam == VK_RETURN || wParam == VK_SPACE)) {
      g_report_key = (int)wParam;
      if (wParam == VK_UP || wParam == VK_DOWN)
        g_report_keep_selection_visible = 1;
      return 0;
    }
    break;
  }
  return DefWindowProc(hwnd, msg, wParam, lParam);
}

int lmr_init_window(int width, int height, const char *title) {
  g_win_width = width;
  g_win_height = height;
  g_should_close = 0;

  WNDCLASS wc = {0};
  wc.lpfnWndProc = lmr_wnd_proc;
  wc.hInstance = GetModuleHandle(NULL);
  wc.lpszClassName = "LmrWindowClass";
  wc.hCursor = LoadCursor(NULL, IDC_ARROW);
  RegisterClass(&wc);

  RECT rc = {0, 0, width, height};
  AdjustWindowRect(&rc, WS_OVERLAPPEDWINDOW, FALSE);

  g_hwnd = CreateWindowEx(
      0, "LmrWindowClass", title,
      WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
      CW_USEDEFAULT, CW_USEDEFAULT, rc.right - rc.left, rc.bottom - rc.top,
      NULL, NULL, GetModuleHandle(NULL), NULL);

  if (!g_hwnd)
    return 0;

  HDC hdc = GetDC(g_hwnd);
  g_hdc_mem = CreateCompatibleDC(hdc);
  g_hbm_mem = CreateCompatibleBitmap(hdc, width, height);
  SelectObject(g_hdc_mem, g_hbm_mem);
  ReleaseDC(g_hwnd, hdc);

  UpdateWindow(g_hwnd);
  return 1;
}

int lmr_window_should_close(void) {
  MSG msg;
  while (PeekMessage(&msg, NULL, 0, 0, PM_REMOVE)) {
    if (msg.message == WM_QUIT) {
      g_should_close = 1;
    }
    TranslateMessage(&msg);
    DispatchMessage(&msg);
  }
  return g_should_close;
}

void lmr_begin_drawing(void) {}

void lmr_clear_background(int r, int g, int b) {
  if (!g_hdc_mem)
    return;
  HBRUSH brush = CreateSolidBrush(RGB(r, g, b));
  RECT rc = {0, 0, g_win_width, g_win_height};
  FillRect(g_hdc_mem, &rc, brush);
  DeleteObject(brush);
}

void lmr_draw_rectangle(int x, int y, int w, int h, int r, int g, int b) {
  if (!g_hdc_mem)
    return;
  HBRUSH brush = CreateSolidBrush(RGB(r, g, b));
  HPEN pen = CreatePen(PS_SOLID, 1, RGB(r, g, b));
  HGDIOBJ old_brush = SelectObject(g_hdc_mem, brush);
  HGDIOBJ old_pen = SelectObject(g_hdc_mem, pen);
  Rectangle(g_hdc_mem, x, y, x + w, y + h);
  SelectObject(g_hdc_mem, old_brush);
  SelectObject(g_hdc_mem, old_pen);
  DeleteObject(brush);
  DeleteObject(pen);
}

void lmr_draw_text(const char *text, int x, int y, int size, int r, int g,
                   int b) {
  if (!g_hdc_mem)
    return;
  SetBkMode(g_hdc_mem, TRANSPARENT);
  SetTextColor(g_hdc_mem, RGB(r, g, b));
  HFONT font =
      CreateFont(size, 0, 0, 0, FW_BOLD, FALSE, FALSE, FALSE, DEFAULT_CHARSET,
                 OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY,
                 DEFAULT_PITCH | FF_DONTCARE, "Arial");
  HGDIOBJ old_font = SelectObject(g_hdc_mem, font);
  TextOut(g_hdc_mem, x, y, text, (int)strlen(text));
  SelectObject(g_hdc_mem, old_font);
  DeleteObject(font);
}

void lmr_draw_scrollable_text(const char *text, int x, int y, int w, int h,
                              int font_size, int r, int g, int b) {
  if (!g_hdc_mem || !text || w <= 0 || h <= 0)
    return;

  const int line_height = font_size + 12;
  int line_count = 1;
  for (const char *p = text; *p; ++p)
    if (*p == '\n')
      ++line_count;
  int content_height = line_count * line_height;
  int max_offset = content_height > h ? content_height - h : 0;

  if (g_mouse_wheel_delta != 0) {
    g_scroll_offset -= (g_mouse_wheel_delta / WHEEL_DELTA) * line_height * 3;
    g_mouse_wheel_delta = 0;
  }
  if (g_scroll_offset < 0)
    g_scroll_offset = 0;
  if (g_scroll_offset > max_offset)
    g_scroll_offset = max_offset;

  RECT clip = {x, y, x + w, y + h};
  g_scroll_viewport = clip;
  int saved = SaveDC(g_hdc_mem);
  IntersectClipRect(g_hdc_mem, clip.left, clip.top, clip.right, clip.bottom);
  SetBkMode(g_hdc_mem, TRANSPARENT);
  SetTextColor(g_hdc_mem, RGB(r, g, b));
  HFONT font = CreateFont(font_size, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                          DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                          CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY,
                          DEFAULT_PITCH | FF_DONTCARE, "Arial");
  HGDIOBJ old_font = SelectObject(g_hdc_mem, font);
  const char *line = text;
  int row = 0;
  for (const char *p = text;; ++p) {
    if (*p == '\n' || *p == '\0') {
      int len = (int)(p - line);
      int draw_y = y + row * line_height - g_scroll_offset;
      if (draw_y + line_height >= y && draw_y < y + h && len > 0)
        TextOutA(g_hdc_mem, x + 10, draw_y, line, len);
      ++row;
      if (*p == '\0')
        break;
      line = p + 1;
    }
  }
  SelectObject(g_hdc_mem, old_font);
  DeleteObject(font);
  RestoreDC(g_hdc_mem, saved);

  HBRUSH track = CreateSolidBrush(RGB(35, 42, 52));
  RECT bar = {x + w - 7, y, x + w, y + h};
  FillRect(g_hdc_mem, &bar, track);
  DeleteObject(track);
  if (max_offset > 0) {
    int thumb_h = h * h / content_height;
    if (thumb_h < 24)
      thumb_h = 24;
    int travel = h - thumb_h;
    int thumb_y = y + (max_offset ? (g_scroll_offset * travel) / max_offset : 0);
    HBRUSH thumb = CreateSolidBrush(RGB(120, 135, 150));
    RECT thumb_rect = {x + w - 7, thumb_y, x + w, thumb_y + thumb_h};
    FillRect(g_hdc_mem, &thumb_rect, thumb);
    DeleteObject(thumb);
  }
}

static void lmr_report_draw_right(HDC dc, const char *start, int len,
                                  int right, int y, COLORREF color) {
  if (len <= 0)
    return;
  SIZE text_size = {0, 0};
  GetTextExtentPoint32A(dc, start, len, &text_size);
  SetTextColor(dc, color);
  TextOutA(dc, right - text_size.cx, y, start, len);
}

static void lmr_report_draw_center(HDC dc, const char *text, int len,
                                   int left, int right, int y,
                                   COLORREF color) {
  if (len <= 0) return;
  SIZE text_size = {0, 0};
  GetTextExtentPoint32A(dc, text, len, &text_size);
  SetTextColor(dc, color);
  TextOutA(dc, left + ((right - left - text_size.cx) / 2), y, text, len);
}

static int lmr_report_contains(const char *text, int len, const char *query) {
  int qlen = (int)strlen(query);
  if (qlen == 0) return 1;
  if (qlen > len) return 0;
  for (int i = 0; i <= len - qlen; ++i) {
    int j = 0;
    while (j < qlen &&
           tolower((unsigned char)text[i + j]) ==
               tolower((unsigned char)query[j]))
      ++j;
    if (j == qlen) return 1;
  }
  return 0;
}

static void lmr_report_split(const char *start, const char *end,
                             const char **fields, int *lengths) {
  const char *p = start;
  for (int i = 0; i < 7; ++i) {
    const char *sep = memchr(p, '\t', (size_t)(end - p));
    fields[i] = p;
    if (!sep) {
      lengths[i] = (int)(end - p);
      for (int j = i + 1; j < 8; ++j) {
        fields[j] = end;
        lengths[j] = 0;
      }
      return;
    }
    lengths[i] = (int)(sep - p);
    p = sep + 1;
  }
  fields[7] = p;
  lengths[7] = (int)(end - p);
}

static int lmr_report_row_visible(const char *start, const char *end) {
  const char *fields[8];
  int lengths[8];
  lmr_report_split(start, end, fields, lengths);
  int won = lmr_report_contains(fields[3], lengths[3], "GANADA");
  int lost = lmr_report_contains(fields[3], lengths[3], "PERDIDA");
  int live = lmr_report_contains(fields[3], lengths[3], "EN VIVO");
  int pending = lmr_report_contains(fields[3], lengths[3], "PENDIENTE");
  if (g_report_filter == 1 && !won) return 0;
  if (g_report_filter == 2 && !lost) return 0;
  if (g_report_filter == 3 && !pending) return 0;
  if (g_report_filter == 4 && !live) return 0;
  return lmr_report_contains(start, (int)(end - start), g_report_search);
}

static void lmr_report_button(HDC dc, RECT rect, const char *label,
                              int selected, int font_size) {
  HBRUSH brush = CreateSolidBrush(selected ? RGB(0, 112, 82) : RGB(48, 57, 68));
  FillRect(dc, &rect, brush);
  DeleteObject(brush);
  SetTextColor(dc, selected ? RGB(240, 253, 250) : RGB(204, 214, 224));
  TextOutA(dc, rect.left + 8, rect.top + 5, label, (int)strlen(label));
  (void)font_size;
}

static int lmr_report_neighbor(const char *data, int rows, int current,
                               int direction) {
  const char *rec = data;
  int candidate = -1;
  for (int i = 0; i < rows && *rec; ++i) {
    const char *end = strchr(rec, '\n');
    if (!end) break;
    if (lmr_report_row_visible(rec, end)) {
      if (direction > 0 && i > current) return i;
      if (direction < 0 && i < current) candidate = i;
      if (direction < 0 && i >= current) break;
    }
    rec = end + 1;
  }
  return candidate;
}

static int lmr_report_index_visible(const char *data, int rows, int wanted) {
  const char *rec = data;
  for (int i = 0; i < rows && *rec; ++i) {
    const char *end = strchr(rec, '\n');
    if (!end) return 0;
    if (i == wanted) return lmr_report_row_visible(rec, end);
    rec = end + 1;
  }
  return 0;
}

void lmr_draw_report_table(const char *data, int x, int y, int w, int h,
                           int font_size) {
  if (!g_hdc_mem || !data || w <= 0 || h <= 0)
    return;
  g_scroll_viewport = (RECT){x, y, x + w, y + h};
  const int line_h = font_size + 10;
  const int header_h = 106;
  const int h2h_header_h = 28;
  const int h2h_row_h = 26;
  int rows = 0;
  for (const char *p = data; *p; ++p)
    if (*p == '\n' && rows < 2048)
      ++rows;

  RECT panel = {x, y, x + w, y + h};
  HBRUSH bg = CreateSolidBrush(RGB(25, 30, 38));
  FillRect(g_hdc_mem, &panel, bg);
  DeleteObject(bg);
  RECT expand_btn = {x + 8, y + 4, x + 126, y + 29};
  RECT collapse_btn = {x + 134, y + 4, x + 258, y + 29};
  SetBkMode(g_hdc_mem, TRANSPARENT);
  SetTextColor(g_hdc_mem, RGB(230, 235, 240));
  HFONT font = CreateFont(font_size, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                          DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                          CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY,
                          DEFAULT_PITCH | FF_DONTCARE, "Arial");
  HGDIOBJ old_font = SelectObject(g_hdc_mem, font);
  HFONT h2h_font = CreateFont(16, 0, 0, 0, FW_NORMAL, FALSE, FALSE, FALSE,
                              DEFAULT_CHARSET, OUT_DEFAULT_PRECIS,
                              CLIP_DEFAULT_PRECIS, DEFAULT_QUALITY,
                              DEFAULT_PITCH | FF_DONTCARE, "Arial");
  lmr_report_button(g_hdc_mem, expand_btn, "Expandir todos", 0, font_size);
  lmr_report_button(g_hdc_mem, collapse_btn, "Contraer todos", 0, font_size);
  RECT filter_btns[5] = {
      {x + 8, y + 38, x + 76, y + 64},
      {x + 82, y + 38, x + 166, y + 64},
      {x + 172, y + 38, x + 256, y + 64},
      {x + 262, y + 38, x + 370, y + 64},
      {x + 376, y + 38, x + 448, y + 64}};
  const char *filter_labels[5] = {"Todos", "Ganadas", "Perdidas", "Pendientes", "En vivo"};
  for (int i = 0; i < 5; ++i)
    lmr_report_button(g_hdc_mem, filter_btns[i], filter_labels[i],
                      g_report_filter == i, font_size);
  RECT search_box = {x + w - 330, y + 4, x + w - 8, y + 30};
  HBRUSH search_bg = CreateSolidBrush(RGB(18, 23, 30));
  FillRect(g_hdc_mem, &search_box, search_bg);
  DeleteObject(search_bg);
  HPEN search_pen = CreatePen(PS_SOLID, 1, g_report_search_focus
                                               ? RGB(0, 210, 150)
                                               : RGB(70, 82, 96));
  HGDIOBJ old_pen = SelectObject(g_hdc_mem, search_pen);
  HGDIOBJ old_brush = SelectObject(g_hdc_mem, GetStockObject(NULL_BRUSH));
  Rectangle(g_hdc_mem, search_box.left, search_box.top, search_box.right,
            search_box.bottom);
  SelectObject(g_hdc_mem, old_pen);
  SelectObject(g_hdc_mem, old_brush);
  DeleteObject(search_pen);
  const char *search_label = g_report_search_len ? g_report_search : "Buscar jugador...";
  SetTextColor(g_hdc_mem, g_report_search_len ? RGB(240, 245, 250)
                                               : RGB(125, 140, 155));
  TextOutA(g_hdc_mem, search_box.left + 9, search_box.top + 5, search_label,
           (int)strlen(search_label));

  const int col_widths[7] = {105, 75, 360, 145, 145, 430, 132};
  const char *col_labels[7] = {"Fecha", "Hora", "Jugadores", "Apuesta",
                               "H2H", "Sets", "Profit"};
  RECT table_header = {x + 4, y + 72, x + w - 10, y + 102};
  HBRUSH table_header_brush = CreateSolidBrush(RGB(34, 41, 50));
  FillRect(g_hdc_mem, &table_header, table_header_brush);
  DeleteObject(table_header_brush);
  int col_right = x + 8;
  for (int i = 0; i < 7; ++i) {
    col_right += col_widths[i];
    lmr_report_draw_right(g_hdc_mem, col_labels[i],
                          (int)strlen(col_labels[i]), col_right - 8,
                          y + 77, RGB(176, 194, 212));
  }

  if (g_mouse_clicked) {
    if (g_mouse_click_x >= expand_btn.left && g_mouse_click_x < expand_btn.right &&
        g_mouse_click_y >= expand_btn.top && g_mouse_click_y < expand_btn.bottom) {
      for (int i = 0; i < rows; ++i) g_report_expanded[i] = 1;
      g_scroll_offset = 0;
    } else if (g_mouse_click_x >= collapse_btn.left &&
               g_mouse_click_x < collapse_btn.right &&
               g_mouse_click_y >= collapse_btn.top &&
               g_mouse_click_y < collapse_btn.bottom) {
      for (int i = 0; i < 2048; ++i) g_report_expanded[i] = 0;
      g_scroll_offset = 0;
    }
    if (g_mouse_click_x >= search_box.left &&
        g_mouse_click_x < search_box.right &&
        g_mouse_click_y >= search_box.top &&
        g_mouse_click_y < search_box.bottom) {
      g_report_search_focus = 1;
      g_report_keyboard_focus = 0;
      if (g_mouse_click_x >= search_box.right - 27) {
        g_report_search_len = 0;
        g_report_search[0] = '\0';
      }
    } else if (g_mouse_click_x >= x && g_mouse_click_x < x + w &&
               g_mouse_click_y >= y && g_mouse_click_y < y + h) {
      g_report_search_focus = 0;
      g_report_keyboard_focus = 1;
    }
    for (int i = 0; i < 5; ++i) {
      if (g_mouse_click_x >= filter_btns[i].left &&
          g_mouse_click_x < filter_btns[i].right &&
          g_mouse_click_y >= filter_btns[i].top &&
          g_mouse_click_y < filter_btns[i].bottom) {
        g_report_filter = i;
        g_scroll_offset = 0;
      }
    }
  }

  if (!g_report_search_focus) {
    if (g_report_selected_row < 0 ||
        !lmr_report_index_visible(data, rows, g_report_selected_row))
      g_report_selected_row = lmr_report_neighbor(data, rows, -1, 1);
    if (g_report_key == VK_UP || g_report_key == VK_DOWN) {
      int next = lmr_report_neighbor(data, rows, g_report_selected_row,
                                     g_report_key == VK_UP ? -1 : 1);
      if (next >= 0) g_report_selected_row = next;
    } else if (g_report_key == VK_LEFT) {
      if (g_report_selected_col > 0) --g_report_selected_col;
    } else if (g_report_key == VK_RIGHT) {
      if (g_report_selected_col < 6) ++g_report_selected_col;
    } else if (g_report_key == VK_RETURN || g_report_key == VK_SPACE) {
      if (g_report_selected_row >= 0 && g_report_selected_row < rows)
        g_report_expanded[g_report_selected_row] =
            !g_report_expanded[g_report_selected_row];
    }
    g_report_key = 0;
  }

  int content_h = 0;
  int selected_y = -1;
  const char *rec = data;
  int idx = 0;
  while (*rec && idx < rows) {
    const char *end = strchr(rec, '\n');
    if (!end) break;
    if (!lmr_report_row_visible(rec, end)) {
      rec = end + 1;
      ++idx;
      continue;
    }
    if (idx == g_report_selected_row) selected_y = content_h;
    content_h += line_h + 7;
    const char *fields[8];
    int lengths[8];
    lmr_report_split(rec, end, fields, lengths);
    if (g_report_expanded[idx] && lengths[7] > 0) {
      content_h += h2h_header_h;
      const char *d = fields[7];
      while (d < end) {
        const char *de = memchr(d, '^', (size_t)(end - d));
        if (!de) de = end;
        content_h += h2h_row_h;
        d = de + 1;
      }
    }
    rec = end + 1;
    ++idx;
  }
  int view_h = h - header_h;
  int max_off = content_h > view_h ? content_h - view_h : 0;
  // Keep the selected row in view only after keyboard navigation. Doing this
  // every frame makes touchpad scrolling jump back to the selected row.
  if (g_report_keep_selection_visible && selected_y >= 0) {
    if (selected_y < g_scroll_offset)
      g_scroll_offset = selected_y;
    else if (selected_y + line_h + 7 > g_scroll_offset + view_h)
      g_scroll_offset = selected_y + line_h + 7 - view_h;
    g_report_keep_selection_visible = 0;
  }
  if (g_mouse_clicked && max_off > 0 && g_mouse_click_x >= x + w - 9 &&
      g_mouse_click_x < x + w && g_mouse_click_y >= y + header_h &&
      g_mouse_click_y < y + h)
    g_scroll_offset = ((g_mouse_click_y - y - header_h) * max_off) / view_h;
  if (g_mouse_wheel_delta) {
    // Preserve partial high-resolution wheel deltas emitted by touchpads.
    int wheel_steps = g_mouse_wheel_delta / WHEEL_DELTA;
    if (wheel_steps != 0) {
      g_scroll_offset -= wheel_steps * line_h * 3;
      g_mouse_wheel_delta -= wheel_steps * WHEEL_DELTA;
    }
  }
  if (g_scroll_offset < 0) g_scroll_offset = 0;
  if (g_scroll_offset > max_off) g_scroll_offset = max_off;

  int saved = SaveDC(g_hdc_mem);
  IntersectClipRect(g_hdc_mem, x + 2, y + header_h, x + w - 9, y + h - 2);
  rec = data;
  idx = 0;
  int cy = y + header_h - g_scroll_offset;
  while (*rec && idx < rows) {
    const char *end = strchr(rec, '\n');
    if (!end) break;
    if (!lmr_report_row_visible(rec, end)) {
      rec = end + 1;
      ++idx;
      continue;
    }
    const char *fields[8];
    int lengths[8];
    lmr_report_split(rec, end, fields, lengths);
    RECT row_bg = {x + 4, cy, x + w - 10, cy + line_h + 7};
    COLORREF row_color = (idx % 2) ? RGB(30, 36, 44) : RGB(34, 41, 50);
    if (idx == g_report_selected_row && g_report_keyboard_focus)
      row_color = RGB(37, 57, 73);
    HBRUSH row_brush = CreateSolidBrush(row_color);
    FillRect(g_hdc_mem, &row_bg, row_brush);
    DeleteObject(row_brush);
    if (g_mouse_clicked && g_mouse_click_x >= x && g_mouse_click_x < x + w - 10 &&
        g_mouse_click_y >= y + header_h && g_mouse_click_y < y + h &&
        g_mouse_click_y >= cy && g_mouse_click_y < cy + line_h + 7) {
      g_report_selected_row = idx;
      int left = x + 8;
      g_report_selected_col = 6;
      for (int c = 0; c < 7; ++c) {
        if (g_mouse_click_x >= left && g_mouse_click_x < left + col_widths[c]) {
          g_report_selected_col = c;
          break;
        }
        left += col_widths[c];
      }
      g_report_expanded[idx] = !g_report_expanded[idx];
    }
    SetTextColor(g_hdc_mem, g_report_expanded[idx] ? RGB(0, 220, 155)
                                                    : RGB(145, 165, 185));
    TextOutA(g_hdc_mem, x + 12, cy + 4, g_report_expanded[idx] ? "[-]" : "[+]", 3);
    COLORREF summary_color = RGB(226, 232, 240);
    if (lmr_report_contains(fields[3], lengths[3], "GANADA"))
      summary_color = RGB(54, 211, 153);
    else if (lmr_report_contains(fields[3], lengths[3], "PERDIDA"))
      summary_color = RGB(248, 113, 113);
    else if (lmr_report_contains(fields[3], lengths[3], "EN VIVO"))
      summary_color = RGB(251, 191, 36);
    int cell_right = x + 8;
    for (int col = 0; col < 7; ++col) {
      cell_right += col_widths[col];
      COLORREF cell_color = RGB(205, 215, 225);
      if (col == 3) cell_color = summary_color;
      if (col == 4) cell_color = RGB(120, 195, 220);
      if (col == 5) cell_color = RGB(185, 198, 212);
      if (col == 6) {
        if (lmr_report_contains(fields[6], lengths[6], "-$") ||
            lmr_report_contains(fields[6], lengths[6], "$-"))
          cell_color = RGB(248, 113, 113);
        else if (!lmr_report_contains(fields[6], lengths[6], "--"))
          cell_color = RGB(54, 211, 153);
      }
      lmr_report_draw_right(g_hdc_mem, fields[col], lengths[col],
                            cell_right - 8, cy + 3, cell_color);
    }
    if (idx == g_report_selected_row && g_report_keyboard_focus) {
      int cell_left = x + 8;
      for (int c = 0; c < g_report_selected_col; ++c)
        cell_left += col_widths[c];
      HPEN focus_pen = CreatePen(PS_SOLID, 1, RGB(0, 210, 150));
      HGDIOBJ prev_pen = SelectObject(g_hdc_mem, focus_pen);
      HGDIOBJ prev_brush = SelectObject(g_hdc_mem, GetStockObject(NULL_BRUSH));
      Rectangle(g_hdc_mem, cell_left, cy + 1,
                cell_left + col_widths[g_report_selected_col] - 1,
                cy + line_h + 6);
      SelectObject(g_hdc_mem, prev_pen);
      SelectObject(g_hdc_mem, prev_brush);
      DeleteObject(focus_pen);
    }
    cy += line_h + 7;
    if (g_report_expanded[idx] && lengths[7] > 0) {
      HGDIOBJ prior_detail_font = SelectObject(g_hdc_mem, h2h_font);
      RECT detail_header = {x + 18, cy, x + w - 18, cy + h2h_header_h};
      HBRUSH detail_header_brush = CreateSolidBrush(RGB(42, 51, 62));
      FillRect(g_hdc_mem, &detail_header, detail_header_brush);
      DeleteObject(detail_header_brush);
      const int detail_widths[4] = {130, 470, 150, 622};
      const char *detail_labels[4] = {"Fecha", "Jugadores", "Marcador", "Sets"};
      int detail_left = x + 24;
      for (int c = 0; c < 4; ++c) {
        lmr_report_draw_center(g_hdc_mem, detail_labels[c],
                               (int)strlen(detail_labels[c]), detail_left,
                               detail_left + detail_widths[c], cy + 6,
                               RGB(168, 190, 210));
        detail_left += detail_widths[c];
      }
      cy += h2h_header_h;
      const char *d = fields[7];
      int detail_index = 0;
      while (d < end) {
        const char *de = memchr(d, '^', (size_t)(end - d));
        if (!de) de = end;
        RECT detail_bg = {x + 18, cy, x + w - 18, cy + h2h_row_h};
        HBRUSH detail_brush = CreateSolidBrush((detail_index % 2)
                                                   ? RGB(28, 34, 42)
                                                   : RGB(32, 39, 48));
        FillRect(g_hdc_mem, &detail_bg, detail_brush);
        DeleteObject(detail_brush);
        const char *detail_fields[4];
        int detail_lengths[4];
        const char *field_start = d;
        for (int c = 0; c < 4; ++c) {
          const char *sep = c < 3
                                ? memchr(field_start, '\t', (size_t)(de - field_start))
                                : NULL;
          detail_fields[c] = field_start;
          if (sep) {
            detail_lengths[c] = (int)(sep - field_start);
            field_start = sep + 1;
          } else {
            detail_lengths[c] = (int)(de - field_start);
            field_start = de;
          }
        }
        detail_left = x + 24;
        for (int c = 0; c < 4; ++c) {
          lmr_report_draw_center(g_hdc_mem, detail_fields[c],
                                 detail_lengths[c], detail_left,
                                 detail_left + detail_widths[c], cy + 4,
                                 RGB(195, 207, 220));
          detail_left += detail_widths[c];
        }
        cy += h2h_row_h;
        d = de + 1;
        ++detail_index;
      }
      SelectObject(g_hdc_mem, prior_detail_font);
    }
    rec = end + 1;
    ++idx;
  }
  RestoreDC(g_hdc_mem, saved);

  HBRUSH track = CreateSolidBrush(RGB(38, 45, 54));
  RECT bar = {x + w - 7, y + header_h, x + w, y + h};
  FillRect(g_hdc_mem, &bar, track);
  DeleteObject(track);
  if (max_off > 0) {
    int thumb_h = view_h * view_h / content_h;
    if (thumb_h < 22) thumb_h = 22;
    int travel = view_h - thumb_h;
    int thumb_y = y + header_h + (g_scroll_offset * travel) / max_off;
    HBRUSH thumb = CreateSolidBrush(RGB(135, 151, 168));
    RECT thumb_rect = {x + w - 7, thumb_y, x + w, thumb_y + thumb_h};
    FillRect(g_hdc_mem, &thumb_rect, thumb);
    DeleteObject(thumb);
  }
  SelectObject(g_hdc_mem, old_font);
  DeleteObject(h2h_font);
  DeleteObject(font);
  if (g_mouse_clicked) g_mouse_clicked = 0;
}

void lmr_end_drawing(void) {
  if (!g_hwnd || !g_hdc_mem)
    return;
  HDC hdc = GetDC(g_hwnd);
  BitBlt(hdc, 0, 0, g_win_width, g_win_height, g_hdc_mem, 0, 0, SRCCOPY);
  ReleaseDC(g_hwnd, hdc);
  Sleep(16); // Límite suave de ~60 FPS
}

void lmr_close_window(void) {
  if (g_hdc_mem)
    DeleteDC(g_hdc_mem);
  if (g_hbm_mem)
    DeleteObject(g_hbm_mem);
  if (g_hwnd)
    DestroyWindow(g_hwnd);
  g_hwnd = NULL;
  g_hdc_mem = NULL;
  g_hbm_mem = NULL;
}
#else
int lmr_init_window(int w, int h, const char *t) {
  (void)w;
  (void)h;
  (void)t;
  return 0;
}
int lmr_window_should_close(void) { return 1; }
void lmr_begin_drawing(void) {}
void lmr_clear_background(int r, int g, int b) {
  (void)r;
  (void)g;
  (void)b;
}
void lmr_draw_rectangle(int x, int y, int w, int h, int r, int g, int b) {
  (void)x;
  (void)y;
  (void)w;
  (void)h;
  (void)r;
  (void)g;
  (void)b;
}
void lmr_draw_text(const char *t, int x, int y, int s, int r, int g, int b) {
  (void)t;
  (void)x;
  (void)y;
  (void)s;
  (void)r;
  (void)g;
  (void)b;
}
void lmr_draw_scrollable_text(const char *t, int x, int y, int w, int h,
                              int s, int r, int g, int b) {
  (void)t; (void)x; (void)y; (void)w; (void)h; (void)s;
  (void)r; (void)g; (void)b;
}
void lmr_draw_report_table(const char *t, int x, int y, int w, int h, int s) {
  (void)t; (void)x; (void)y; (void)w; (void)h; (void)s;
}
void lmr_end_drawing(void) {}
void lmr_close_window(void) {}
#endif
// ============================================================
// 7. MEMORIA COMPARTIDA ENTRE HILOS (INTER-THREAD CACHE)
// ============================================================
static double g_price_btc = 84000.0;
static double g_price_eth = 3200.0;
static double g_price_sol = 190.0;

void lmr_set_crypto_price(const char *symbol, double price) {
  if (!symbol)
    return;
  if (strstr(symbol, "BTC"))
    g_price_btc = price;
  else if (strstr(symbol, "ETH"))
    g_price_eth = price;
  else if (strstr(symbol, "SOL"))
    g_price_sol = price;
}

double lmr_get_crypto_price(const char *symbol) {
  if (!symbol)
    return 0.0;
  if (strstr(symbol, "BTC"))
    return g_price_btc;
  else if (strstr(symbol, "ETH"))
    return g_price_eth;
  else if (strstr(symbol, "SOL"))
    return g_price_sol;
  return 0.0;
}
// ============================================================
// 8. TECLADO EN TIEMPO REAL
// ============================================================
#ifdef _WIN32
int lmr_is_key_down(int vkey) {
  return (GetAsyncKeyState(vkey) & 0x8000) ? 1 : 0;
}
#else
int lmr_is_key_down(int vkey) {
  (void)vkey;
  return 0;
}
#endif

// ============================================================
// 9. CONECTOR NATIVO SQLITE (Sin descargas, usa winsqlite3.dll)
// ============================================================
#ifdef _WIN32
typedef int (*fn_sql_open)(const char *, void **);
typedef int (*fn_sql_open_v2)(const char *, void **, int, const char *);
typedef int (*fn_sql_close)(void *);
typedef int (*fn_sql_exec)(void *, const char *, void *, void *, char **);
typedef int (*fn_sql_prepare)(void *, const char *, int, void **,
                              const char **);
typedef int (*fn_sql_step)(void *);
typedef const unsigned char *(*fn_sql_text)(void *, int);
typedef long long (*fn_sql_int64)(void *, int);
typedef double (*fn_sql_double)(void *, int);
typedef int (*fn_sql_finalize)(void *);
typedef int (*fn_sql_bind_text)(void *, int, const char *, int, void (*)(void *));
typedef int (*fn_sql_bind_int)(void *, int, int);
typedef int (*fn_sql_bind_double)(void *, int, double);
typedef int (*fn_sql_bind_null)(void *, int);
typedef int (*fn_sql_reset)(void *);

static HMODULE g_hSqlite = NULL;
static fn_sql_open p_sqlite3_open = NULL;
static fn_sql_open_v2 p_sqlite3_open_v2 = NULL;
static fn_sql_close p_sqlite3_close = NULL;
static fn_sql_exec p_sqlite3_exec = NULL;
static fn_sql_prepare p_sqlite3_prepare_v2 = NULL;
static fn_sql_step p_sqlite3_step = NULL;
static fn_sql_text p_sqlite3_column_text = NULL;
static fn_sql_int64 p_sqlite3_column_int64 = NULL;
static fn_sql_double p_sqlite3_column_double = NULL;
static fn_sql_finalize p_sqlite3_finalize = NULL;
static fn_sql_bind_text p_sqlite3_bind_text = NULL;
static fn_sql_bind_int p_sqlite3_bind_int = NULL;
static fn_sql_bind_double p_sqlite3_bind_double = NULL;
static fn_sql_bind_null p_sqlite3_bind_null = NULL;
static fn_sql_reset p_sqlite3_reset = NULL;

static int lmr_sqlite_init(void) {
  if (g_hSqlite)
    return 1;
  g_hSqlite = LoadLibraryA("winsqlite3.dll");
  if (!g_hSqlite) {
    g_hSqlite = LoadLibraryA("sqlite3.dll");
  }
  if (!g_hSqlite)
    return 0;

  p_sqlite3_open = (fn_sql_open)GetProcAddress(g_hSqlite, "sqlite3_open");
  p_sqlite3_open_v2 =
      (fn_sql_open_v2)GetProcAddress(g_hSqlite, "sqlite3_open_v2");
  p_sqlite3_close = (fn_sql_close)GetProcAddress(g_hSqlite, "sqlite3_close");
  p_sqlite3_exec = (fn_sql_exec)GetProcAddress(g_hSqlite, "sqlite3_exec");
  p_sqlite3_prepare_v2 =
      (fn_sql_prepare)GetProcAddress(g_hSqlite, "sqlite3_prepare_v2");
  p_sqlite3_step = (fn_sql_step)GetProcAddress(g_hSqlite, "sqlite3_step");
  p_sqlite3_column_text =
      (fn_sql_text)GetProcAddress(g_hSqlite, "sqlite3_column_text");
  p_sqlite3_column_int64 =
      (fn_sql_int64)GetProcAddress(g_hSqlite, "sqlite3_column_int64");
  p_sqlite3_column_double =
      (fn_sql_double)GetProcAddress(g_hSqlite, "sqlite3_column_double");
  p_sqlite3_finalize =
      (fn_sql_finalize)GetProcAddress(g_hSqlite, "sqlite3_finalize");
  p_sqlite3_bind_text =
      (fn_sql_bind_text)GetProcAddress(g_hSqlite, "sqlite3_bind_text");
  p_sqlite3_bind_int =
      (fn_sql_bind_int)GetProcAddress(g_hSqlite, "sqlite3_bind_int");
  p_sqlite3_bind_double =
      (fn_sql_bind_double)GetProcAddress(g_hSqlite, "sqlite3_bind_double");
  p_sqlite3_bind_null =
      (fn_sql_bind_null)GetProcAddress(g_hSqlite, "sqlite3_bind_null");
  p_sqlite3_reset =
      (fn_sql_reset)GetProcAddress(g_hSqlite, "sqlite3_reset");

  return (p_sqlite3_open && p_sqlite3_open_v2 && p_sqlite3_close && p_sqlite3_prepare_v2 &&
          p_sqlite3_step);
}
#endif

void *lmr_db_open(const char *path) {
#ifdef _WIN32
  if (!lmr_sqlite_init())
    return NULL;
  void *db = NULL;
  if (p_sqlite3_open(path, &db) == 0) {
    return db;
  }
  return NULL;
#else
  return NULL;
#endif
}

void *lmr_db_open_readonly(const char *path) {
#ifdef _WIN32
  if (!lmr_sqlite_init())
    return NULL;
  void *db = NULL;
  const int SQLITE_OPEN_READONLY = 0x00000001;
  if (p_sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, NULL) == 0)
    return db;
  return NULL;
#else
  (void)path;
  return NULL;
#endif
}

int lmr_db_is_valid(void *handle) { return handle != NULL; }

void *lmr_db_close(void *db) {
#ifdef _WIN32
  if (db && p_sqlite3_close) {
    p_sqlite3_close(db);
  }
#endif
  return NULL;
}

int lmr_db_execute(void *db, const char *sql) {
#ifdef _WIN32
  if (!db || !p_sqlite3_exec)
    return 0;
  return (p_sqlite3_exec(db, sql, NULL, NULL, NULL) == 0);
#else
  return 0;
#endif
}

void *lmr_db_prepare(void *db, const char *sql) {
#ifdef _WIN32
  if (!db || !p_sqlite3_prepare_v2)
    return NULL;
  void *stmt = NULL;
  if (p_sqlite3_prepare_v2(db, sql, -1, &stmt, NULL) == 0) {
    return stmt;
  }
  return NULL;
#else
  return NULL;
#endif
}

int lmr_db_step(void *stmt) {
#ifdef _WIN32
  if (!stmt || !p_sqlite3_step)
    return 0;
  // SQLITE_ROW == 100 en SQLite
  return (p_sqlite3_step(stmt) == 100);
#else
  return 0;
#endif
}

char *lmr_db_get_text(void *stmt, int col) {
#ifdef _WIN32
  if (!stmt || !p_sqlite3_column_text) {
    char *empty = (char *)malloc(1);
    empty[0] = '\0';
    return empty;
  }
  const unsigned char *txt = p_sqlite3_column_text(stmt, col);
  if (!txt) {
    char *empty = (char *)malloc(1);
    empty[0] = '\0';
    return empty;
  }
  size_t len = strlen((const char *)txt);
  char *res = (char *)malloc(len + 1);
  memcpy(res, txt, len);
  res[len] = '\0';
  return res;
#else
  char *empty = (char *)malloc(1);
  empty[0] = '\0';
  return empty;
#endif
}

long long lmr_db_get_int(void *stmt, int col) {
#ifdef _WIN32
  if (!stmt || !p_sqlite3_column_int64)
    return 0;
  return p_sqlite3_column_int64(stmt, col);
#else
  return 0;
#endif
}

double lmr_db_get_float(void *stmt, int col) {
#ifdef _WIN32
  if (!stmt || !p_sqlite3_column_double)
    return 0.0;
  return p_sqlite3_column_double(stmt, col);
#else
  return 0.0;
#endif
}

void *lmr_db_finalize(void *stmt) {
#ifdef _WIN32
  if (stmt && p_sqlite3_finalize) {
    p_sqlite3_finalize(stmt);
  }
#endif
  return NULL;
}

// Importa tickets Bet365 de un CSV en la tabla separada apuestas_bet365.
// El identificador (archivo de origen, orden) permite repetir la importación
// sin duplicar tickets; si el CSV se corrigió, actualiza esa fila.
typedef struct {
  char **fields;
  size_t count;
  size_t capacity;
} LmrCsvRow;

static void lmr_csv_row_free(LmrCsvRow *row) {
  if (!row) return;
  for (size_t i = 0; i < row->count; ++i) free(row->fields[i]);
  free(row->fields);
  row->fields = NULL;
  row->count = 0;
  row->capacity = 0;
}

static int lmr_csv_push_field(LmrCsvRow *row, const char *value) {
  if (row->count == row->capacity) {
    size_t next = row->capacity ? row->capacity * 2 : 16;
    char **grown = (char **)realloc(row->fields, next * sizeof(char *));
    if (!grown) return 0;
    row->fields = grown;
    row->capacity = next;
  }
  size_t length = strlen(value);
  row->fields[row->count] = (char *)malloc(length + 1);
  if (!row->fields[row->count]) return 0;
  memcpy(row->fields[row->count], value, length + 1);
  row->count++;
  return 1;
}

// CSV RFC4180: acepta campos entre comillas, comillas escapadas y saltos
// de línea dentro de un campo.
static int lmr_csv_next_record(const char **cursor, LmrCsvRow *row) {
  if (!cursor || !*cursor || !**cursor) return 0;
  memset(row, 0, sizeof(*row));
  size_t capacity = 128, length = 0;
  char *field = (char *)malloc(capacity);
  if (!field) return -1;
  int quoted = 0, started = 0, saw_data = 0;
  const char *p = *cursor;
  while (*p) {
    char c = *p++;
    saw_data = 1;
    if (quoted) {
      if (c == '"') {
        if (*p == '"') {
          ++p;
          c = '"';
        } else {
          quoted = 0;
          continue;
        }
      }
    } else if (c == '"' && !started && length == 0) {
      quoted = 1;
      started = 1;
      continue;
    } else if (c == ',') {
      field[length] = '\0';
      if (!lmr_csv_push_field(row, field)) goto memory_error;
      length = 0;
      started = 0;
      continue;
    } else if (c == '\n' || c == '\r') {
      if (c == '\r' && *p == '\n') ++p;
      field[length] = '\0';
      if (!lmr_csv_push_field(row, field)) goto memory_error;
      free(field);
      *cursor = p;
      return 1;
    } else {
      started = 1;
    }
    if (length + 2 > capacity) {
      size_t next = capacity * 2;
      char *grown = (char *)realloc(field, next);
      if (!grown) goto memory_error;
      field = grown;
      capacity = next;
    }
    field[length++] = c;
  }
  if (quoted) {
    free(field);
    return -1;
  }
  if (!saw_data && row->count == 0) {
    free(field);
    return 0;
  }
  field[length] = '\0';
  if (!lmr_csv_push_field(row, field)) goto memory_error;
  free(field);
  *cursor = p;
  return 1;

memory_error:
  free(field);
  lmr_csv_row_free(row);
  return -1;
}

static char *lmr_open_csv_utf8(const char *path, long *size_out) {
  if (!path || !size_out) return NULL;
  FILE *file = NULL;
#ifdef _WIN32
  int wide_length = MultiByteToWideChar(CP_UTF8, 0, path, -1, NULL, 0);
  if (wide_length <= 0) return NULL;
  wchar_t *wide_path = (wchar_t *)malloc((size_t)wide_length * sizeof(wchar_t));
  if (!wide_path) return NULL;
  MultiByteToWideChar(CP_UTF8, 0, path, -1, wide_path, wide_length);
  file = _wfopen(wide_path, L"rb");
  free(wide_path);
#else
  file = fopen(path, "rb");
#endif
  if (!file) return NULL;
  if (fseek(file, 0, SEEK_END) != 0) { fclose(file); return NULL; }
  long size = ftell(file);
  if (size < 0 || fseek(file, 0, SEEK_SET) != 0) { fclose(file); return NULL; }
  char *contents = (char *)malloc((size_t)size + 1);
  if (!contents) { fclose(file); return NULL; }
  size_t read_size = fread(contents, 1, (size_t)size, file);
  fclose(file);
  if (read_size != (size_t)size) { free(contents); return NULL; }
  contents[size] = '\0';
  *size_out = size;
  return contents;
}

static int lmr_csv_header_index(const LmrCsvRow *header, const char *name) {
  for (size_t i = 0; i < header->count; ++i) {
    const char *field = header->fields[i];
    if (i == 0 && (unsigned char)field[0] == 0xEF &&
        (unsigned char)field[1] == 0xBB && (unsigned char)field[2] == 0xBF)
      field += 3;
    while (*field == ' ' || *field == '\t') ++field;
    size_t length = strlen(field);
    while (length && (field[length - 1] == ' ' || field[length - 1] == '\t'))
      --length;
    if (strlen(name) == length && LMR_STRNICMP(field, name, length) == 0)
      return (int)i;
  }
  return -1;
}

static const char *lmr_csv_value(const LmrCsvRow *row, const int *map,
                                 int column) {
  int index = map[column];
  return index >= 0 && (size_t)index < row->count ? row->fields[index] : "";
}

static int lmr_csv_number(const char *text, double *value, int required) {
  if (!text || !*text) {
    if (!required) { *value = 0.0; return 0; }
    return -1;
  }
  errno = 0;
  char *end = NULL;
  double parsed = strtod(text, &end);
  while (end && (*end == ' ' || *end == '\t')) ++end;
  if (errno || end == text || (end && *end) || !isfinite(parsed)) return -1;
  *value = parsed;
  return 1;
}

#define LMR_SQLITE_TRANSIENT ((void (*)(void *))-1)

int lmr_import_bet365_csv(const char *csv_path, const char *db_path) {
#ifdef _WIN32
  if (!lmr_sqlite_init() || !p_sqlite3_bind_text || !p_sqlite3_bind_int ||
      !p_sqlite3_bind_double || !p_sqlite3_bind_null || !p_sqlite3_reset ||
      !p_sqlite3_finalize || !p_sqlite3_exec) return -1;
  long csv_size = 0;
  char *contents = lmr_open_csv_utf8(csv_path, &csv_size);
  if (!contents) { fprintf(stderr, "ERROR: no se pudo leer el CSV Bet365.\n"); return -2; }
  (void)csv_size;
  void *db = lmr_db_open(db_path);
  if (!db) { free(contents); fprintf(stderr, "ERROR: no se pudo abrir SQLite.\n"); return -3; }

  const char *table_sql =
      "CREATE TABLE IF NOT EXISTS apuestas_bet365 ("
      "id INTEGER PRIMARY KEY AUTOINCREMENT,"
      "source_file TEXT NOT NULL, source_order INTEGER NOT NULL,"
      "fecha TEXT NOT NULL, hora TEXT NOT NULL, fecha_hora_original TEXT,"
      "deporte TEXT NOT NULL, seleccion TEXT NOT NULL, tipo_ou TEXT,"
      "linea REAL, cuota REAL NOT NULL CHECK(cuota > 1.0),"
      "importe REAL NOT NULL CHECK(importe >= 0), ganancias REAL, neto REAL,"
      "estado TEXT NOT NULL, bank_previo REAL, bank_acumulado REAL,"
      "imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,"
      "UNIQUE(source_file, source_order))";
  if (p_sqlite3_exec(db, "BEGIN IMMEDIATE", NULL, NULL, NULL) != 0 ||
      p_sqlite3_exec(db, table_sql, NULL, NULL, NULL) != 0) {
    p_sqlite3_exec(db, "ROLLBACK", NULL, NULL, NULL);
    lmr_db_close(db); free(contents);
    fprintf(stderr, "ERROR: no se pudo crear la tabla apuestas_bet365.\n");
    return -4;
  }

  const char *insert_sql =
      "INSERT INTO apuestas_bet365 (source_file,source_order,fecha,hora,"
      "fecha_hora_original,deporte,seleccion,tipo_ou,linea,cuota,importe,"
      "ganancias,neto,estado,bank_previo,bank_acumulado)"
      " VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
      " ON CONFLICT(source_file,source_order) DO UPDATE SET"
      " fecha=excluded.fecha,hora=excluded.hora,"
      " fecha_hora_original=excluded.fecha_hora_original,"
      " deporte=excluded.deporte,seleccion=excluded.seleccion,"
      " tipo_ou=excluded.tipo_ou,linea=excluded.linea,cuota=excluded.cuota,"
      " importe=excluded.importe,ganancias=excluded.ganancias,"
      " neto=excluded.neto,estado=excluded.estado,"
      " bank_previo=excluded.bank_previo,bank_acumulado=excluded.bank_acumulado,"
      " imported_at=CURRENT_TIMESTAMP";
  void *stmt = NULL;
  if (p_sqlite3_prepare_v2(db, insert_sql, -1, &stmt, NULL) != 0 || !stmt) {
    p_sqlite3_exec(db, "ROLLBACK", NULL, NULL, NULL);
    lmr_db_close(db); free(contents);
    fprintf(stderr, "ERROR: no se pudo preparar la importacion SQLite.\n");
    return -5;
  }

  const char *column_names[] = {"orden","fecha","hora","fecha_hora_original",
      "deporte","seleccion","tipo_ou","linea","cuota","importe",
      "ganancias","neto","estado","bank_previo","bank_acumulado"};
  int map[15];
  const char *cursor = contents;
  LmrCsvRow header;
  int record_status = lmr_csv_next_record(&cursor, &header);
  if (record_status != 1) {
    p_sqlite3_finalize(stmt); p_sqlite3_exec(db,"ROLLBACK",NULL,NULL,NULL);
    lmr_db_close(db); free(contents); lmr_csv_row_free(&header);
    fprintf(stderr, "ERROR: el CSV no tiene una cabecera valida.\n");
    return -6;
  }
  for (int i = 0; i < 15; ++i) map[i] = lmr_csv_header_index(&header, column_names[i]);
  lmr_csv_row_free(&header);
  for (int i = 0; i < 13; ++i) {
    if (map[i] < 0) {
      p_sqlite3_finalize(stmt); p_sqlite3_exec(db,"ROLLBACK",NULL,NULL,NULL);
      lmr_db_close(db); free(contents);
      fprintf(stderr, "ERROR: falta la columna requerida '%s'.\n", column_names[i]);
      return -6;
    }
  }

  const char *base_name = strrchr(csv_path, '/');
  const char *windows_name = strrchr(csv_path, '\\');
  if (!base_name || (windows_name && windows_name > base_name)) base_name = windows_name;
  base_name = base_name ? base_name + 1 : csv_path;
  int imported = 0, skipped = 0, line = 1, failure = 0;
  LmrCsvRow row;
  while ((record_status = lmr_csv_next_record(&cursor, &row)) > 0) {
    ++line;
    if (row.count == 1 && row.fields[0][0] == '\0') { lmr_csv_row_free(&row); continue; }
    const char *sport = lmr_csv_value(&row, map, 4);
    if (_stricmp(sport, "Tenis de Mesa") != 0) {
      ++skipped;
      lmr_csv_row_free(&row);
      continue;
    }
    double values[7] = {0};
    const int numeric_columns[] = {0,7,8,9,10,11,13,14};
    const int numeric_map[] = {0,7,8,9,10,11,13,14};
    for (int i = 0; i < 8; ++i) {
      const char *value = lmr_csv_value(&row, map, numeric_map[i]);
      int required = (numeric_columns[i] == 0 || numeric_columns[i] == 8 || numeric_columns[i] == 9);
      double parsed = 0.0;
      int parsed_status = lmr_csv_number(value, &parsed, required);
      if (parsed_status < 0) { failure = 1; break; }
      if (numeric_columns[i] == 0) values[0] = parsed;
      else if (numeric_columns[i] == 7) values[1] = parsed;
      else if (numeric_columns[i] == 8) values[2] = parsed;
      else if (numeric_columns[i] == 9) values[3] = parsed;
      else if (numeric_columns[i] == 10) values[4] = parsed;
      else if (numeric_columns[i] == 11) values[5] = parsed;
      else if (numeric_columns[i] == 13) values[6] = parsed;
    }
    double bank_acumulado = 0.0;
    int bank_status = lmr_csv_number(lmr_csv_value(&row,map,14), &bank_acumulado, 0);
    double bank_previo = 0.0;
    int prior_status = lmr_csv_number(lmr_csv_value(&row,map,13), &bank_previo, 0);
    (void)bank_status; (void)prior_status;
    int order = (int)values[0];
    if (failure || order <= 0 || values[2] <= 1.0 || values[3] < 0.0) {
      failure = 1;
      lmr_csv_row_free(&row);
      break;
    }
    const char *texts[] = {base_name,
      lmr_csv_value(&row,map,1), lmr_csv_value(&row,map,2),
      lmr_csv_value(&row,map,3), sport, lmr_csv_value(&row,map,5),
      lmr_csv_value(&row,map,6), lmr_csv_value(&row,map,12)};
    int bind_ok = 1;
    bind_ok &= p_sqlite3_bind_text(stmt,1,texts[0],-1,LMR_SQLITE_TRANSIENT)==0;
    bind_ok &= p_sqlite3_bind_int(stmt,2,order)==0;
    for (int i = 0; i < 6; ++i)
      bind_ok &= p_sqlite3_bind_text(stmt,3+i,texts[1+i],-1,LMR_SQLITE_TRANSIENT)==0;
    if (map[7] >= 0 && lmr_csv_value(&row,map,7)[0]) bind_ok &= p_sqlite3_bind_double(stmt,9,values[1])==0;
    else bind_ok &= p_sqlite3_bind_null(stmt,9)==0;
    bind_ok &= p_sqlite3_bind_double(stmt,10,values[2])==0;
    bind_ok &= p_sqlite3_bind_double(stmt,11,values[3])==0;
    if (lmr_csv_value(&row,map,10)[0]) bind_ok &= p_sqlite3_bind_double(stmt,12,values[4])==0;
    else bind_ok &= p_sqlite3_bind_null(stmt,12)==0;
    if (lmr_csv_value(&row,map,11)[0]) bind_ok &= p_sqlite3_bind_double(stmt,13,values[5])==0;
    else bind_ok &= p_sqlite3_bind_null(stmt,13)==0;
    bind_ok &= p_sqlite3_bind_text(stmt,14,texts[7],-1,LMR_SQLITE_TRANSIENT)==0;
    if (lmr_csv_value(&row,map,13)[0]) bind_ok &= p_sqlite3_bind_double(stmt,15,bank_previo)==0;
    else bind_ok &= p_sqlite3_bind_null(stmt,15)==0;
    if (lmr_csv_value(&row,map,14)[0]) bind_ok &= p_sqlite3_bind_double(stmt,16,bank_acumulado)==0;
    else bind_ok &= p_sqlite3_bind_null(stmt,16)==0;
    if (!bind_ok || p_sqlite3_step(stmt) != 101) failure = 1;
    p_sqlite3_reset(stmt);
    if (failure) {
      fprintf(stderr, "ERROR: fila CSV invalida o fallo al guardar (fila %d).\n", line);
      lmr_csv_row_free(&row);
      break;
    }
    ++imported;
    lmr_csv_row_free(&row);
  }
  if (record_status < 0) failure = 1;
  p_sqlite3_finalize(stmt);
  free(contents);
  if (failure || p_sqlite3_exec(db,"COMMIT",NULL,NULL,NULL) != 0) {
    p_sqlite3_exec(db,"ROLLBACK",NULL,NULL,NULL);
    lmr_db_close(db);
    fprintf(stderr, "ERROR: importacion cancelada; la transaccion se revirtio.\n");
    return -7;
  }
  lmr_db_close(db);
  printf("Importacion Bet365: %d tickets de tenis de mesa guardados/actualizados; %d de otros deportes omitidos.\n", imported, skipped);
  return imported;
#else
  (void)csv_path; (void)db_path;
  return -1;
#endif
}

// ============================================================
// 10. CLIENTE SETKA CUP: calendario + H2H (JSON y SQLite)
// ============================================================
// Parser JSON pequeño para las respuestas del API de Setka. Conserva la
// estructura de objetos/arreglos y soporta escapes comunes en strings.
typedef struct { int parent, start, end, next; char type; } SetkaJsonToken;
typedef struct { const char *src; int pos, count, cap; SetkaJsonToken *t; } SetkaJson;

static void sj_ws(SetkaJson *j) {
  while (j->src[j->pos] == ' ' || j->src[j->pos] == '\t' ||
         j->src[j->pos] == '\r' || j->src[j->pos] == '\n') j->pos++;
}
static int sj_value(SetkaJson *j, int parent) {
  sj_ws(j); if (!j->src[j->pos] || j->count >= j->cap) return -1;
  int id = j->count++; SetkaJsonToken *t = &j->t[id];
  t->parent = parent; t->start = j->pos; t->end = -1; t->next = -1;
  char c = j->src[j->pos++];
  if (c == '{' || c == '[') {
    t->type = c;
    for (;;) {
      sj_ws(j); char x = j->src[j->pos];
      if (!x) return -1;
      if ((c == '{' && x == '}') || (c == '[' && x == ']')) { j->pos++; break; }
      if (x == ',' || x == ':') { j->pos++; continue; }
      if (sj_value(j, id) < 0) return -1;
    }
  } else if (c == '"') {
    t->type = 's';
    while (j->src[j->pos]) {
      if (j->src[j->pos] == '\\' && j->src[j->pos + 1]) { j->pos += 2; continue; }
      if (j->src[j->pos++] == '"') break;
    }
  } else {
    t->type = 'p';
    while (j->src[j->pos] && j->src[j->pos] != ',' && j->src[j->pos] != ']' &&
           j->src[j->pos] != '}' && j->src[j->pos] != ' ' && j->src[j->pos] != '\n') j->pos++;
  }
  t->end = j->pos; t->next = j->count; return id;
}
static int sj_parse(SetkaJson *j, const char *src) {
  j->src = src; j->pos = 0; j->count = 0; j->cap = 120000;
  j->t = (SetkaJsonToken *)calloc((size_t)j->cap, sizeof(SetkaJsonToken));
  if (!j->t) return -1;
  return sj_value(j, -1);
}
static int sj_prop(SetkaJson *j, int obj, const char *key) {
  if (obj < 0 || j->t[obj].type != '{') return -1;
  for (int i = obj + 1; i < j->t[obj].next;) {
    int val = i + 1;
    if (j->t[i].type == 's') {
      int n = j->t[i].end - j->t[i].start - 2;
      if ((int)strlen(key) == n && !strncmp(j->src + j->t[i].start + 1, key, (size_t)n)) return val;
    }
    i = j->t[val].next;
  }
  return -1;
}
static int sj_at(SetkaJson *j, int arr, int wanted) {
  if (arr < 0 || j->t[arr].type != '[') return -1;
  int n = 0;
  for (int i = arr + 1; i < j->t[arr].next; i = j->t[i].next) if (n++ == wanted) return i;
  return -1;
}
static char *sj_text(SetkaJson *j, int id) {
  if (id < 0) { char *empty=(char *)malloc(1); if(empty)empty[0]=0; return empty; }
  SetkaJsonToken *t = &j->t[id]; int a = t->start, b = t->end;
  if (t->type == 's') { a++; b--; }
  char *out = (char *)malloc((size_t)(b - a) + 1); if (!out) return NULL;
  int k = 0;
  for (int i = a; i < b; i++) {
    char c = j->src[i];
    if (c == '\\' && i + 1 < b) { c = j->src[++i]; if (c == 'n') c='\n'; else if (c=='r') c='\r'; else if (c=='t') c='\t'; }
    out[k++] = c;
  }
  out[k] = 0; return out;
}
static long long sj_num(SetkaJson *j, int id) { char *s=sj_text(j,id); long long n=s?atoll(s):0; free(s); return n; }
static char *sj_prop_text(SetkaJson *j, int obj, const char *key) { return sj_text(j, sj_prop(j,obj,key)); }
static void sj_free(SetkaJson *j) { free(j->t); j->t=NULL; }
static char *sj_sql(const char *s) {
  size_t n=2; for (const char *p=s; *p; p++) n += (*p=='\'') ? 2 : 1;
  char *q=(char *)malloc(n+1); size_t k=0; q[k++]='\'';
  for (const char *p=s; *p; p++) { q[k++]=*p; if (*p=='\'') q[k++]='\''; }
  q[k++]='\''; q[k]=0; return q;
}
static int sj_date_local(const char *iso, char *dt, size_t dtcap, char *day, size_t daycap, char *hour, size_t hourcap) {
  int y=0,mo=0,d=0,hh=0,mm=0,ss=0,offh=0,offm=0; char sign='+';
  if (sscanf(iso,"%d-%d-%dT%d:%d:%d",&y,&mo,&d,&hh,&mm,&ss)<5) return 0;
  const char *zone=strchr(iso,'T'); if (zone) { const char *z=strpbrk(zone,"+-"); if (z && z>zone) { sign=*z; sscanf(z+1,"%d:%d",&offh,&offm); } }
  struct tm tmv; memset(&tmv,0,sizeof(tmv)); tmv.tm_year=y-1900; tmv.tm_mon=mo-1; tmv.tm_mday=d; tmv.tm_hour=hh; tmv.tm_min=mm; tmv.tm_sec=ss;
#ifdef _WIN32
  time_t epoch=_mkgmtime(&tmv);
#else
  time_t epoch=timegm(&tmv);
#endif
  int offset=(offh*60+offm)*(sign=='-'?-1:1); epoch += (time_t)(-offset-300)*60;
  struct tm local_tm;
#ifdef _WIN32
  gmtime_s(&local_tm,&epoch);
#else
  gmtime_r(&epoch,&local_tm);
#endif
  strftime(dt,dtcap,"%Y-%m-%dT%H:%M:%S-05:00",&local_tm);
  strftime(day,daycap,"%d.%m.%y",&local_tm); strftime(hour,hourcap,"%H:%M",&local_tm); return 1;
}
static const char *sj_arenas(long long id) {
  switch(id) { case 2:return "Africa";case 3:return "Europe";case 4:return "Australia";case 5:return "America";case 6:return "Asia";case 7:return "Seoul";case 8:return "Tokyo";case 9:return "Beijing";case 10:return "London";case 13:return "Rome";case 14:return "Paris";case 18:return "Mexico";case 19:return "Rio";case 30:return "Montreal";default:return NULL; }
}
static char *sj_http_get(const char *url, int *http_status) {
  if (http_status) *http_status=0;
  if (!lmr_curl_url_is_safe(url, 0)) return NULL;
  char cmd[2600];
  int cmd_len=snprintf(cmd,sizeof(cmd),"curl --proto =http,https --proto-redir =http,https -sS -L --retry 2 --retry-delay 1 --connect-timeout 10 --max-time 30 -A \"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/126 Safari/537.36\" -e \"https://tabletennis.setkacup.com/en/schedule\" -H \"Accept: application/json, text/plain, */*\" -w \"\\n%%{http_code}\" -- \"%s\"",url);
  if(cmd_len<0||(size_t)cmd_len>=sizeof(cmd))return NULL;
  FILE *fp=POPEN(cmd,"r"); if(!fp){if(http_status)*http_status=0;return NULL;}
  size_t cap=8192,len=0;char *buf=(char *)malloc(cap);if(!buf){PCLOSE(fp);if(http_status)*http_status=0;return NULL;}
  char chunk[4096];while(fgets(chunk,sizeof(chunk),fp)){size_t n=strlen(chunk);if(len+n+1>cap){cap=(len+n+1)*2;char *b=(char *)realloc(buf,cap);if(!b){free(buf);PCLOSE(fp);if(http_status)*http_status=0;return NULL;}buf=b;}memcpy(buf+len,chunk,n);len+=n;}buf[len]=0;PCLOSE(fp);
  if(len>=3&&buf[len-1]=='\n')buf[--len]=0;int code=0;if(len>=3){code=atoi(buf+len-3);len-=3;if(len&&buf[len-1]=='\n')len--;buf[len]=0;}if(http_status)*http_status=code;return buf;
}
#ifdef _WIN32
typedef struct { long long a,b; char *body; int status; HANDLE thread; } SetkaH2hFetch;
static DWORD WINAPI sj_fetch_h2h_thread(LPVOID data) {
  SetkaH2hFetch *task=(SetkaH2hFetch *)data; char url[512];
  snprintf(url,sizeof(url),"https://tabletennis.setkacup.com/api/Players/en/compare?player1Id=%lld&player2Id=%lld",task->a,task->b);
  task->body=sj_http_get(url,&task->status); return 0;
}
#endif
static int sj_save_match(void *db, SetkaJson *j, int m) {
  long long id=sj_num(j,sj_prop(j,m,"id")), status=sj_num(j,sj_prop(j,m,"statusId"));
  int p1=sj_prop(j,m,"player1"),p2=sj_prop(j,m,"player2");
  long long p1id=sj_num(j,sj_prop(j,p1,"id")),p2id=sj_num(j,sj_prop(j,p2,"id"));
  long long winner=sj_num(j,sj_prop(j,sj_prop(j,m,"winner"),"id"));
  long long s1=sj_num(j,sj_prop(j,m,"player1Score")),s2=sj_num(j,sj_prop(j,m,"player2Score"));
  char *start=sj_prop_text(j,m,"startDate"),dt[64],day[24],hour[16];
  if (!id || !p1id || !p2id || !sj_date_local(start,dt,sizeof(dt),day,sizeof(day),hour,sizeof(hour))) { free(start); return 0; }
  long long loc=sj_num(j,sj_prop(j,m,"locationId")); const char *arena=sj_arenas(loc);
  if (!arena) { free(start); return 0; }
  char *f1=sj_prop_text(j,p1,"firstName"),*l1=sj_prop_text(j,p1,"lastName"),*f2=sj_prop_text(j,p2,"firstName"),*l2=sj_prop_text(j,p2,"lastName");
  char name1[512],name2[512],score[64]; snprintf(name1,sizeof(name1),"%s %s",f1,l1);snprintf(name2,sizeof(name2),"%s %s",f2,l2);snprintf(score,sizeof(score),"%lld : %lld",s1,s2);
  char sets1[4096]="[",sets2[4096]="["; int arr=sj_prop(j,m,"setScores"),first=1;
  for(int i=0;(arr>=0)&&( (i==0)?sj_at(j,arr,i)>=0:1 );i++) { int x=sj_at(j,arr,i); if(x<0)break; long long ix=sj_num(j,sj_prop(j,x,"number"));
    // Orden esperado por number; se guarda en el orden recibido y se ordena en API.
    long long a=sj_num(j,sj_prop(j,x,"p1Score")),b=sj_num(j,sj_prop(j,x,"p2Score")); char q[64];
    snprintf(q,sizeof(q),"%s%lld",first?"":",",a);strncat(sets1,q,sizeof(sets1)-strlen(sets1)-1);
    snprintf(q,sizeof(q),"%s%lld",first?"":",",b);strncat(sets2,q,sizeof(sets2)-strlen(sets2)-1);first=0;(void)ix;
  }
  strncat(sets1,"]",sizeof(sets1)-strlen(sets1)-1);strncat(sets2,"]",sizeof(sets2)-strlen(sets2)-1);
  char *qdt=sj_sql(dt),*qday=sj_sql(day),*qhour=sj_sql(hour),*qarena=sj_sql(arena),*qn1=sj_sql(name1),*qn2=sj_sql(name2),*qscore=sj_sql(score),*qsets1=sj_sql(sets1),*qsets2=sj_sql(sets2);
  int winner_token=sj_prop(j,sj_prop(j,m,"winner"),"id");
  char *winner_text=sj_text(j,winner_token); char winner_sql[48];
  if(winner_token<0 || !winner_text || !strcmp(winner_text,"null"))snprintf(winner_sql,sizeof(winner_sql),"NULL");else snprintf(winner_sql,sizeof(winner_sql),"%lld",winner);
  char sql[16000]; snprintf(sql,sizeof(sql),"INSERT INTO partidos(match_id,status,winner_id,datetime_local,fecha,hora_gmt5,arena,p1_id,p2_id,jugador_1,jugador_2,score_p1,score_p2,p1_sets,p2_sets,marcador) VALUES(%lld,%lld,%s,%s,%s,%s,%s,%lld,%lld,%s,%s,%lld,%lld,%s,%s,%s) ON CONFLICT(match_id) DO UPDATE SET status=excluded.status,winner_id=excluded.winner_id,datetime_local=excluded.datetime_local,fecha=excluded.fecha,hora_gmt5=excluded.hora_gmt5,arena=excluded.arena,p1_id=excluded.p1_id,p2_id=excluded.p2_id,jugador_1=excluded.jugador_1,jugador_2=excluded.jugador_2,score_p1=excluded.score_p1,score_p2=excluded.score_p2,p1_sets=excluded.p1_sets,p2_sets=excluded.p2_sets,marcador=excluded.marcador",id,status,winner_sql,qdt,qday,qhour,qarena,p1id,p2id,qn1,qn2,s1,s2,qsets1,qsets2,qscore);
  int ok=lmr_db_execute(db,sql);
  free(start);free(f1);free(l1);free(f2);free(l2);free(winner_text);free(qdt);free(qday);free(qhour);free(qarena);free(qn1);free(qn2);free(qscore);free(qsets1);free(qsets2);return ok;
}

// Devuelve partidos insertados/actualizados de calendario + H2H. La API del
// proyecto original se consulta directamente; no depende de Python/Playwright.
int lmr_setka_refresh(const char *db_path, const char *date_iso) {
  // This bridge routine creates worker threads on Windows. Keep the collector
  // disabled for this process so it never sweeps concurrently mutated data.
  lmr_disable_gc();
#ifdef _WIN32
  printf("[Setka] Base de datos: %s | Fecha: %s\n",db_path,date_iso);fflush(stdout);
  if (!lmr_sqlite_init()) return -1;
  void *db=NULL; if (p_sqlite3_open(db_path,&db)!=0 || !db) return -2;
  lmr_db_execute(db,"CREATE TABLE IF NOT EXISTS partidos(match_id INTEGER PRIMARY KEY,status INTEGER,winner_id INTEGER,datetime_local TEXT,fecha TEXT,hora_gmt5 TEXT,arena TEXT,p1_id INTEGER,p2_id INTEGER,jugador_1 TEXT,jugador_2 TEXT,score_p1 INTEGER,score_p2 INTEGER,p1_sets TEXT,p2_sets TEXT,marcador TEXT,origen TEXT DEFAULT 'setka',b365_line REAL,b365_c1 REAL,b365_c2 REAL,b365_c_over REAL,b365_c_under REAL)");
  lmr_db_execute(db,"CREATE INDEX IF NOT EXISTS idx_partidos_fecha ON partidos(fecha)");
  lmr_db_execute(db,"CREATE INDEX IF NOT EXISTS idx_partidos_p1 ON partidos(p1_id,status,datetime_local)");
  lmr_db_execute(db,"CREATE INDEX IF NOT EXISTS idx_partidos_p2 ON partidos(p2_id,status,datetime_local)");
  lmr_db_execute(db,"CREATE INDEX IF NOT EXISTS idx_partidos_h2h ON partidos(p1_id,p2_id,status,datetime_local)");
  lmr_db_execute(db,"CREATE TABLE IF NOT EXISTS h2h_consultado(p1_id INTEGER,p2_id INTEGER,dia TEXT,PRIMARY KEY(p1_id,p2_id))");
  char url[512]; int saved=0,complete=1;
  for(int period=1;period<=7;period++) {
    printf("[Setka] Descargando periodo %d/7...\n",period);fflush(stdout);
    snprintf(url,sizeof(url),"https://tabletennis.setkacup.com/api/Tournaments/en?date=%s&dayPeriod=%d",date_iso,period);
    int http=0;char *body=sj_http_get(url,&http); if(http==204||http==404){free(body);continue;}if(http!=200||!body||!*body){complete=0;printf("[Setka] Periodo %d fallo (HTTP %d).\n",period,http);fflush(stdout);free(body);continue;}
    SetkaJson j; int root=sj_parse(&j,body); if(root<0){complete=0;printf("[Setka] No se pudo interpretar JSON del periodo %d.\n",period);fflush(stdout);free(body);continue;}
    if(j.t[root].type!='['){printf("[Setka] Respuesta inesperada del periodo %d (tipo %c).\n",period,j.t[root].type);fflush(stdout);}
    int before_period=saved;
    for(int ti=0;;ti++){int tr=sj_at(&j,root,ti);if(tr<0)break;int loc=(int)sj_num(&j,sj_prop(&j,tr,"locationId"));if(!sj_arenas(loc))continue;int ms=sj_prop(&j,tr,"matches");
      for(int mi=0;;mi++){int m=sj_at(&j,ms,mi);if(m<0)break;saved+=sj_save_match(db,&j,m);}}
    printf("[Setka] Periodo %d: %d partidos insertados/actualizados.\n",period,saved-before_period);fflush(stdout);
    sj_free(&j);free(body);
  }
  // Consultar cruces del día que todavía no están marcados en h2h_consultado.
  char *qdate=sj_sql(date_iso); char select_sql[512];snprintf(select_sql,sizeof(select_sql),"SELECT DISTINCT p1_id,p2_id FROM partidos WHERE fecha=(strftime('%%d.%%m.', '%s') || substr('%s',3,2)) AND status IN (1,2,3) AND p1_id IS NOT NULL AND p2_id IS NOT NULL",date_iso,date_iso);
  void *st=lmr_db_prepare(db,select_sql); long long pairs_a[4096],pairs_b[4096];int np=0;
  while(st&&lmr_db_step(st)&&np<4096){long long a=p_sqlite3_column_int64(st,0),b=p_sqlite3_column_int64(st,1);if(a>b){long long z=a;a=b;b=z;}pairs_a[np]=a;pairs_b[np++]=b;} if(st)lmr_db_finalize(st);
  printf("[Setka] Consultando H2H de hasta %d parejas...\n",np);fflush(stdout);
  for(int base=0;base<np;base+=8){
    SetkaH2hFetch batch[8];HANDLE handles[8];int count=np-base;if(count>8)count=8;int active=0;
    memset(batch,0,sizeof(batch));
    for(int j=0;j<count;j++){int i=base+j;batch[j].a=pairs_a[i];batch[j].b=pairs_b[i];char check[256];snprintf(check,sizeof(check),"SELECT 1 FROM h2h_consultado WHERE p1_id=%lld AND p2_id=%lld",batch[j].a,batch[j].b);void *cs=lmr_db_prepare(db,check);int exists=cs&&lmr_db_step(cs);if(cs)lmr_db_finalize(cs);if(exists)continue;batch[j].thread=CreateThread(NULL,0,sj_fetch_h2h_thread,&batch[j],0,NULL);if(batch[j].thread)handles[active++]=batch[j].thread;}
    if(active)WaitForMultipleObjects((DWORD)active,handles,TRUE,INFINITE);
    printf("[Setka] H2H %d-%d/%d\n",base+1,base+count,np);fflush(stdout);
    for(int j=0;j<count;j++){long long a=batch[j].a,b=batch[j].b;if(batch[j].thread)CloseHandle(batch[j].thread);if(!batch[j].thread){char check[256];snprintf(check,sizeof(check),"SELECT 1 FROM h2h_consultado WHERE p1_id=%lld AND p2_id=%lld",a,b);void *cs=lmr_db_prepare(db,check);int exists=cs&&lmr_db_step(cs);if(cs)lmr_db_finalize(cs);if(exists)continue;batch[j].body=NULL;batch[j].status=0;}
      char *body=batch[j].body;int http=batch[j].status;if(body&&http==200){SetkaJson jdata;int root=sj_parse(&jdata,body);if(root>=0){int ms=sj_prop(&jdata,root,"matches");for(int mi=0;;mi++){int m=sj_at(&jdata,ms,mi);if(m<0)break;saved+=sj_save_match(db,&jdata,m);}sj_free(&jdata);}else complete=0;}else if(http!=204&&http!=404)complete=0;free(body);
      char ins[256];snprintf(ins,sizeof(ins),"INSERT OR REPLACE INTO h2h_consultado(p1_id,p2_id,dia) VALUES(%lld,%lld,%s)",a,b,qdate);lmr_db_execute(db,ins);
    }
  }
  free(qdate);p_sqlite3_close(db);return complete?saved:-3;
#else
  (void)db_path;(void)date_iso;return -4;
#endif
}

int lmr_setka_refresh_today(const char *db_path) {
  time_t now=time(NULL); struct tm lima;
#ifdef _WIN32
  now -= 5*60*60; gmtime_s(&lima,&now);
#else
  now -= 5*60*60; gmtime_r(&now,&lima);
#endif
  char day[32]; strftime(day,sizeof(day),"%Y-%m-%d",&lima);
  return lmr_setka_refresh(db_path,day);
}

const char *lmr_setka_choose_db(const char *preferred, const char *fallback) {
#ifdef _WIN32
  static char selected[1024];
  const char *path=(_access(preferred,0)==0 || _access(fallback,0)!=0)?preferred:fallback;
  snprintf(selected,sizeof(selected),"%s",path);
  return selected;
#else
  return preferred;
#endif
}

// Opciones del informe LMR: fechas, franja horaria, composición y stakes.
static int g_report_args_ready = 0;
static char g_report_start_date[16];
static char g_report_end_date[16];
static char g_report_start_time[8] = "00:00";
// 24:00 makes the default window include every match on the selected date.
static char g_report_end_time[8] = "24:00";
static int g_report_compound = 0;
static double g_report_stake_over = 1.0;
static double g_report_stake_under = 1.0;

static int lmr_report_parse_date(const char *value, char *out, size_t out_size) {
  int day = 0, month = 0, year = 0;
  if (sscanf(value, "%d/%d/%d", &day, &month, &year) == 3) {
    // DD/MM/YYYY
  } else if (sscanf(value, "%d-%d-%d", &year, &month, &day) != 3) {
    return 0;
  }
  if (year < 100) year += 2000;
  if (year < 2000 || year > 2099 || month < 1 || month > 12 ||
      day < 1 || day > 31)
    return 0;
  snprintf(out, out_size, "%04d-%02d-%02d", year, month, day);
  return 1;
}

static void lmr_report_init_args(void) {
  if (g_report_args_ready) return;
  g_report_args_ready = 1;
  time_t lima_now = time(NULL) - 5 * 60 * 60;
  struct tm lima_date;
#ifdef _WIN32
  gmtime_s(&lima_date, &lima_now);
#else
  gmtime_r(&lima_now, &lima_date);
#endif
  strftime(g_report_start_date, sizeof(g_report_start_date), "%Y-%m-%d",
           &lima_date);
  snprintf(g_report_end_date, sizeof(g_report_end_date), "%s",
           g_report_start_date);

#ifdef _WIN32
  int argc = 0;
  LPWSTR *wide_argv = CommandLineToArgvW(GetCommandLineW(), &argc);
  char argv[64][256];
  int arg_count = 0;
  if (wide_argv) {
    for (int i = 1; i < argc && arg_count < 64; ++i) {
      WideCharToMultiByte(CP_UTF8, 0, wide_argv[i], -1, argv[arg_count],
                          (int)sizeof(argv[arg_count]), NULL, NULL);
      ++arg_count;
    }
    LocalFree(wide_argv);
  }
#else
  int arg_count = 0;
  char argv[64][256] = {{0}};
#endif

  for (int i = 0; i < arg_count; ++i) {
    if (!strcmp(argv[i], "--date") && i + 1 < arg_count) {
      char parsed[16];
      if (lmr_report_parse_date(argv[i + 1], parsed, sizeof(parsed))) {
        snprintf(g_report_start_date, sizeof(g_report_start_date), "%s", parsed);
        snprintf(g_report_end_date, sizeof(g_report_end_date), "%s", parsed);
      }
      ++i;
    } else if (!strcmp(argv[i], "--start-date") && i + 1 < arg_count) {
      lmr_report_parse_date(argv[i + 1], g_report_start_date,
                            sizeof(g_report_start_date));
      ++i;
    } else if (!strcmp(argv[i], "--end-date") && i + 1 < arg_count) {
      lmr_report_parse_date(argv[i + 1], g_report_end_date,
                            sizeof(g_report_end_date));
      ++i;
    } else if (!strcmp(argv[i], "--start-time") && i + 1 < arg_count) {
      snprintf(g_report_start_time, sizeof(g_report_start_time), "%s", argv[++i]);
    } else if (!strcmp(argv[i], "--end-time") && i + 1 < arg_count) {
      snprintf(g_report_end_time, sizeof(g_report_end_time), "%s", argv[++i]);
    } else if (!strcmp(argv[i], "--compound")) {
      g_report_compound = 1;
    } else if (!strcmp(argv[i], "--stake") && i + 1 < arg_count) {
      double value = atof(argv[++i]);
      if (i + 1 < arg_count && !strcmp(argv[i + 1], "over")) {
        g_report_stake_over = value;
        ++i;
      } else if (i + 1 < arg_count && !strcmp(argv[i + 1], "under")) {
        g_report_stake_under = value;
        ++i;
      } else {
        g_report_stake_over = value;
        g_report_stake_under = value;
      }
    }
  }
}

const char *lmr_report_start_date(void) {
  lmr_report_init_args();
  return g_report_start_date;
}
const char *lmr_report_end_date(void) {
  lmr_report_init_args();
  return g_report_end_date;
}
const char *lmr_report_start_time(void) {
  lmr_report_init_args();
  return g_report_start_time;
}
const char *lmr_report_end_time(void) {
  lmr_report_init_args();
  return g_report_end_time;
}
int lmr_report_compound(void) {
  lmr_report_init_args();
  return g_report_compound;
}
double lmr_report_stake_over(void) {
  lmr_report_init_args();
  return g_report_stake_over;
}
double lmr_report_stake_under(void) {
  lmr_report_init_args();
  return g_report_stake_under;
}

static int sj_array_item_int(const char *json, int wanted) {
  if (!json || *json != '[' || wanted < 0) return -1;
  const char *p=json+1; int index=0;
  while (*p) {
    while (*p==' '||*p=='\t'||*p=='\r'||*p=='\n'||*p==',') p++;
    if (*p==']'||!*p) return -1;
    int quoted=(*p=='"'); if(quoted)p++;
    char *end=NULL; long value=strtol(p,&end,10);
    if (end==p) return -1;
    if (index==wanted) return (int)value;
    index++; p=end; if(quoted&&*p=='"')p++;
    while (*p==' '||*p=='\t'||*p=='\r'||*p=='\n') p++;
    if (*p!=',') return -1;
  }
  return -1;
}
int lmr_set_total(const char *p1_json, const char *p2_json, int set_index) {
  int p1=sj_array_item_int(p1_json,set_index),p2=sj_array_item_int(p2_json,set_index);
  // 0 es el centinela estable para un set inexistente: int de C es i32,
  // pero los enteros LMR se transportan como i64 y -1 se extendía sin signo.
  return (p1<=0||p2<=0)?0:p1+p2;
}
int lmr_set_point(const char *sets_json, int set_index) {
  int point = sj_array_item_int(sets_json, set_index);
  return point > 0 ? point : 0;
}

#ifdef LMR_BRIDGE_URL_TEST
int main(void) {
  if (!lmr_curl_url_is_safe("https://api.example.com/path?x=1&y=two", 0)) return 1;
  if (!lmr_curl_url_is_safe("http://localhost:8080/health", 0)) return 2;
  if (!lmr_curl_url_is_safe("wss://stream.example.com/ws?channel=trade", 1)) return 3;
  if (!lmr_curl_url_is_safe("ws://localhost:9000/socket", 1)) return 4;
  if (lmr_curl_url_is_safe("file:///etc/passwd", 0)) return 5;
  if (lmr_curl_url_is_safe("https://user:password@example.com/", 0)) return 6;
  if (lmr_curl_url_is_safe("https://example.com/\"&calc&\"", 0)) return 7;
  if (lmr_curl_url_is_safe("https://example.com/?q=$(calc)", 0)) return 8;
  if (lmr_curl_url_is_safe("https://example.com/%PATH%", 0)) return 9;
  if (lmr_curl_url_is_safe("https://example.com/\nnext", 0)) return 10;
  if (lmr_curl_url_is_safe("https://example.com/socket", 1)) return 11;
  puts("Native URL guard OK");
  return 0;
}
#endif
