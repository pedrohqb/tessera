/* LD_PRELOAD spy: logs calls that come from libserproidp11.so to
 * $SPY_LOG (or stderr): environment, identity and process queries. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <pwd.h>
#include <sys/utsname.h>
#include <stdarg.h>
#include <time.h>

static __thread int busy;

static int from_driver(void *ret) {
    Dl_info i;
    if (!ret || !dladdr(ret, &i) || !i.dli_fname) return 0;
    return strstr(i.dli_fname, "serproid") != NULL;
}

static void logf_(const char *fmt, ...) {
    static FILE *out;
    if (!out) {
        const char *p = getenv("SPY_LOG");
        out = p ? fopen(p, "a") : NULL;
        if (!out) out = stderr;
        setvbuf(out, NULL, _IOLBF, 0);
    }
    va_list ap; va_start(ap, fmt);
    fprintf(out, "spy[%d] ", (int)getpid());
    vfprintf(out, fmt, ap);
    fputc('\n', out);
    va_end(ap);
}

#define REAL(name) static __typeof__(name) *real_##name; if (!real_##name) real_##name = dlsym(RTLD_NEXT, #name)
#define SHOW(s) ((s) ? (s) : "(null)")

char *getenv(const char *n) {
    REAL(getenv);
    char *v = real_getenv(n);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getenv(%s) = %s", n, SHOW(v)); busy = 0; }
    return v;
}
char *secure_getenv(const char *n) {
    REAL(secure_getenv);
    char *v = real_secure_getenv(n);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("secure_getenv(%s) = %s", n, SHOW(v)); busy = 0; }
    return v;
}
pid_t getpid(void) {
    REAL(getpid);
    pid_t v = real_getpid();
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getpid() = %d", v); busy = 0; }
    return v;
}
pid_t getppid(void) {
    REAL(getppid);
    pid_t v = real_getppid();
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getppid() = %d", v); busy = 0; }
    return v;
}
int gethostname(char *b, size_t l) {
    REAL(gethostname);
    int v = real_gethostname(b, l);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("gethostname() = %s", v == 0 ? b : "?"); busy = 0; }
    return v;
}
int uname(struct utsname *u) {
    REAL(uname);
    int v = real_uname(u);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("uname() = %s %s %s %s", u->sysname, u->nodename, u->release, u->machine); busy = 0; }
    return v;
}
char *getlogin(void) {
    REAL(getlogin);
    char *v = real_getlogin();
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getlogin() = %s", SHOW(v)); busy = 0; }
    return v;
}
int getlogin_r(char *b, size_t l) {
    REAL(getlogin_r);
    int v = real_getlogin_r(b, l);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getlogin_r() = %d %s", v, v == 0 ? b : ""); busy = 0; }
    return v;
}
char *getcwd(char *b, size_t l) {
    REAL(getcwd);
    char *v = real_getcwd(b, l);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getcwd() = %s", SHOW(v)); busy = 0; }
    return v;
}
int isatty(int fd) {
    REAL(isatty);
    int v = real_isatty(fd);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("isatty(%d) = %d", fd, v); busy = 0; }
    return v;
}
ssize_t readlink(const char *p, char *b, size_t l) {
    REAL(readlink);
    ssize_t v = real_readlink(p, b, l);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("readlink(%s) = %.*s", p, v > 0 ? (int)v : 0, b); busy = 0; }
    return v;
}
struct passwd *getpwuid(uid_t u) {
    REAL(getpwuid);
    struct passwd *v = real_getpwuid(u);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getpwuid(%d) = %s", u, v ? v->pw_name : "(null)"); busy = 0; }
    return v;
}
int getpwuid_r(uid_t u, struct passwd *p, char *b, size_t l, struct passwd **r) {
    REAL(getpwuid_r);
    int v = real_getpwuid_r(u, p, b, l, r);
    if (!busy && from_driver(__builtin_return_address(0))) { busy = 1; logf_("getpwuid_r(%d) = %s", u, (*r) ? (*r)->pw_name : "(null)"); busy = 0; }
    return v;
}
