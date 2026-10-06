// rmtrace - LD_PRELOAD logger for the NVIDIA RM control calls NVML makes.
//
// NVML talks to the kernel driver with ioctl() on /dev/nvidiactl and /dev/nvidia*. Every clock, offset and power-limit
// change nvoc asks for ends up as an RM control (NV_ESC_RM_CONTROL, an NVOS54_PARAMETERS block): a command id, the
// object it targets, and a parameter blob. This logs each of those - the blob before and after the call - plus every
// object allocation, so a command's handle can be resolved to its class (0x2080 is the subdevice).
//
// The point is to learn the private command ids and parameter layouts so they can be replayed elsewhere (macuda sends
// RM controls straight to GSP). Diff runs that differ in one setting to find which bytes carry the value.
//
// Build:  cc -shared -fPIC -O2 -o rmtrace.so rmtrace.c -ldl -lpthread
// Run:    sudo env LD_PRELOAD=$PWD/rmtrace.so RMTRACE_OUT=/tmp/oc.log nvoc -o 200
//         (plain `sudo LD_PRELOAD=...` does not work: sudo strips LD_* from the environment.)
//
// Env:    RMTRACE_OUT   log path (appended to); default stderr
//         RMTRACE_MAX   max parameter bytes dumped per call; default 4096, 0 = no limit
//         RMTRACE_ALLOC set to 0 to omit allocation lines
//
// Limits: parameter blobs are dumped flat. A control whose parameters embed a user pointer (an NvP64 to a list) shows
// the pointer value, not what it points at. Calls flagged FINN_SERIALIZED carry a serialized, not raw, layout.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <time.h>
#include <unistd.h>

// From open-gpu-kernel-modules: nv-ioctl-numbers.h, nv_escape.h, nvos.h, nv-ioctl.h.
#define NV_IOCTL_MAGIC         'F'
#define NV_ESC_IOCTL_XFER_CMD  (200 + 11)
#define NV_ESC_RM_CONTROL      0x2A
#define NV_ESC_RM_ALLOC        0x2B
#define NVOS54_FLAGS_FINN_SERIALIZED 0x4

typedef struct { uint32_t cmd, size; uint64_t ptr; } nv_ioctl_xfer_t;
typedef struct {
  uint32_t hClient, hObject, cmd, flags;
  uint64_t params;
  uint32_t paramsSize, status;
} NVOS54_PARAMETERS;
typedef struct {
  uint32_t hRoot, hObjectParent, hObjectNew, hClass;
  uint64_t pAllocParms;
  uint32_t paramsSize, status;
} NVOS21_PARAMETERS;
typedef struct {
  uint32_t hRoot, hObjectParent, hObjectNew, hClass;
  uint64_t pAllocParms, pRightsRequested;
  uint32_t paramsSize, flags, status;
} NVOS64_PARAMETERS;

_Static_assert(sizeof(NVOS54_PARAMETERS) == 32, "NVOS54 layout");
_Static_assert(sizeof(NVOS21_PARAMETERS) == 32, "NVOS21 layout");
_Static_assert(sizeof(NVOS64_PARAMETERS) == 48, "NVOS64 layout");

typedef int (*ioctl_fn)(int, unsigned long, ...);

static ioctl_fn real_ioctl;
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static FILE *out;
static size_t max_dump = 4096;
static int log_alloc = 1;
static unsigned long seq;
static struct timespec t0;

// handle -> class, filled from successful allocations. Handles are per client, so the key is the pair.
#define NCLASS 4096
static struct { uint32_t client, handle, cls; } classes[NCLASS];
static size_t nclasses;

static void init(void) {
  real_ioctl = (ioctl_fn)dlsym(RTLD_NEXT, "ioctl");
  const char *p = getenv("RMTRACE_OUT");
  out = p && *p ? fopen(p, "a") : NULL;
  if (!out) out = stderr;
  setvbuf(out, NULL, _IOLBF, 0);
  if ((p = getenv("RMTRACE_MAX")) && *p) max_dump = strtoul(p, NULL, 0);
  if (max_dump == 0) max_dump = SIZE_MAX;
  if ((p = getenv("RMTRACE_ALLOC")) && *p) log_alloc = atoi(p) != 0;
  clock_gettime(CLOCK_MONOTONIC, &t0);
  char exe[512] = "?";
  ssize_t n = readlink("/proc/self/exe", exe, sizeof(exe) - 1);
  if (n > 0) exe[n] = 0;
  fprintf(out, "# rmtrace pid %d exe %s\n", (int)getpid(), exe);
}

static pthread_once_t once = PTHREAD_ONCE_INIT;

// Open the log at load, not at the first NVIDIA call: an empty log then means the library loaded but saw no calls, and
// a missing one means it never loaded.
__attribute__((constructor)) static void load(void) { pthread_once(&once, init); }

static double elapsed(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return (double)(t.tv_sec - t0.tv_sec) + (double)(t.tv_nsec - t0.tv_nsec) / 1e9;
}

static int is_nvidia_fd(int fd) {
  char link[64], path[256];
  snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
  ssize_t n = readlink(link, path, sizeof(path) - 1);
  if (n <= 0) return 0;
  path[n] = 0;
  return strncmp(path, "/dev/nvidia", 11) == 0;
}

static uint32_t class_of(uint32_t client, uint32_t handle) {
  if (handle == client) return 0x0;  // NV01_ROOT: the client itself
  for (size_t i = nclasses; i-- > 0;)
    if (classes[i].client == client && classes[i].handle == handle) return classes[i].cls;
  return 0xffffffff;
}

static void remember(uint32_t client, uint32_t handle, uint32_t cls) {
  if (nclasses < NCLASS) classes[nclasses++] = (typeof(classes[0])){client, handle, cls};
}

// Little-endian u32 words, eight per line with the byte offset, so values read directly and two runs diff by line.
static void dump(const char *tag, const uint8_t *p, size_t len) {
  size_t n = len < max_dump ? len : max_dump;
  for (size_t off = 0; off < n; off += 32) {
    fprintf(out, "  %s %04zx:", tag, off);
    for (size_t i = off; i < off + 32 && i < n; i += 4) {
      if (i + 4 <= n) {
        uint32_t w;
        memcpy(&w, p + i, 4);
        fprintf(out, " %08x", w);
      } else {
        fputc(' ', out);
        for (size_t j = i; j < n; j++) fprintf(out, "%02x", p[j]);
      }
    }
    fputc('\n', out);
  }
  if (n < len) fprintf(out, "  %s ... %zu more bytes not dumped\n", tag, len - n);
}

static void log_control(NVOS54_PARAMETERS *a, const uint8_t *before, int rc, int err) {
  const uint8_t *after = (const uint8_t *)(uintptr_t)a->params;
  uint32_t cls = class_of(a->hClient, a->hObject);
  fprintf(out, "ctrl #%lu t=%.6f cmd=0x%08x client=0x%08x object=0x%08x", ++seq, elapsed(), a->cmd, a->hClient,
          a->hObject);
  if (cls != 0xffffffff) fprintf(out, " class=0x%04x", cls);
  fprintf(out, " flags=0x%x%s size=%u status=0x%x", a->flags,
          (a->flags & NVOS54_FLAGS_FINN_SERIALIZED) ? "(finn)" : "", a->paramsSize, a->status);
  if (rc < 0) fprintf(out, " ioctl=-1 errno=%d", err);
  fputc('\n', out);
  if (!a->paramsSize || !after) return;
  if (before) {
    dump("in ", before, a->paramsSize);
    if (memcmp(before, after, a->paramsSize)) dump("out", after, a->paramsSize);
    else fprintf(out, "  out unchanged\n");
  } else {
    dump("out", after, a->paramsSize);
  }
}

static void log_alloc_call(int rc, uint32_t client, uint32_t parent, uint32_t handle, uint32_t cls, uint32_t status,
                           const void *params, uint32_t size) {
  if (rc == 0 && status == 0) remember(client, handle, cls);
  if (!log_alloc) return;
  fprintf(out, "alloc #%lu t=%.6f class=0x%04x client=0x%08x parent=0x%08x new=0x%08x size=%u status=0x%x\n", ++seq,
          elapsed(), cls, client, parent, handle, size, status);
  if (size && params) dump("prm", params, size);
}

// One RM escape: `nr` is the escape number, `arg` the parameter struct, `size` its size.
static int traced(int fd, unsigned long req, void *ioarg, unsigned nr, void *arg, unsigned size) {
  if (nr == NV_ESC_RM_CONTROL && size == sizeof(NVOS54_PARAMETERS)) {
    NVOS54_PARAMETERS *a = arg;
    uint8_t *before = NULL;
    if (a->params && a->paramsSize) {
      before = malloc(a->paramsSize);
      if (before) memcpy(before, (void *)(uintptr_t)a->params, a->paramsSize);
    }
    int rc = real_ioctl(fd, req, ioarg);
    int err = errno;
    pthread_mutex_lock(&lock);
    log_control(a, before, rc, err);
    pthread_mutex_unlock(&lock);
    free(before);
    errno = err;
    return rc;
  }
  if (nr == NV_ESC_RM_ALLOC && (size == sizeof(NVOS21_PARAMETERS) || size == sizeof(NVOS64_PARAMETERS))) {
    int rc = real_ioctl(fd, req, ioarg);
    int err = errno;
    pthread_mutex_lock(&lock);
    if (size == sizeof(NVOS64_PARAMETERS)) {
      NVOS64_PARAMETERS *a = arg;
      log_alloc_call(rc, a->hRoot, a->hObjectParent, a->hObjectNew, a->hClass, a->status,
                     (void *)(uintptr_t)a->pAllocParms, a->paramsSize);
    } else {
      NVOS21_PARAMETERS *a = arg;
      log_alloc_call(rc, a->hRoot, a->hObjectParent, a->hObjectNew, a->hClass, a->status,
                     (void *)(uintptr_t)a->pAllocParms, a->paramsSize);
    }
    pthread_mutex_unlock(&lock);
    errno = err;
    return rc;
  }
  return real_ioctl(fd, req, ioarg);
}

int ioctl(int fd, unsigned long req, ...) {
  va_list ap;
  va_start(ap, req);
  void *arg = va_arg(ap, void *);
  va_end(ap);
  pthread_once(&once, init);
  if (_IOC_TYPE(req) != NV_IOCTL_MAGIC || !arg || !is_nvidia_fd(fd)) return real_ioctl(fd, req, arg);
  unsigned nr = _IOC_NR(req);
  // Parameter blocks too large for the _IOC size field go through an indirection; unwrap it.
  if (nr == NV_ESC_IOCTL_XFER_CMD && _IOC_SIZE(req) == sizeof(nv_ioctl_xfer_t)) {
    nv_ioctl_xfer_t *x = arg;
    return traced(fd, req, arg, x->cmd, (void *)(uintptr_t)x->ptr, x->size);
  }
  return traced(fd, req, arg, nr, arg, _IOC_SIZE(req));
}
