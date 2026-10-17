/* A small C library for makine programs: memory, strings, formatted
 * output to the console, and an in-memory file system.
 *
 * Files: a program may embed read-only files with mk_add_file() (e.g. a WAD
 * linked in as a binary blob). Files opened for writing live in RAM until
 * the program ends.
 */
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

#include "makine.h"

int errno;

/* ------------------------------------------------------------------ */
/* Memory: a first-fit free list over the heap between _end and the stack. */

extern char _end[];
extern char __heap_end[];

typedef struct Block {
    size_t size; /* bytes after the header, multiple of 8 */
    struct Block *next; /* next free block, when free */
} Block;

static Block *free_list;
static char *brk_ptr;

static void *sbrk_(size_t n) {
    if (!brk_ptr) brk_ptr = (char *)(((uintptr_t)_end + 15) & ~(uintptr_t)15);
    if (brk_ptr + n > __heap_end) return 0;
    void *p = brk_ptr;
    brk_ptr += n;
    return p;
}

void *malloc(size_t n) {
    if (n == 0) n = 1;
    n = (n + 7) & ~(size_t)7;
    Block **prev = &free_list;
    for (Block *b = free_list; b; prev = &b->next, b = b->next) {
        if (b->size >= n) {
            if (b->size >= n + sizeof(Block) + 16) {
                Block *rest = (Block *)((char *)(b + 1) + n);
                rest->size = b->size - n - sizeof(Block);
                rest->next = b->next;
                *prev = rest;
                b->size = n;
            } else {
                *prev = b->next;
            }
            return b + 1;
        }
    }
    Block *b = sbrk_(sizeof(Block) + n);
    if (!b) {
        errno = ENOMEM;
        return 0;
    }
    b->size = n;
    return b + 1;
}

void free(void *p) {
    if (!p) return;
    Block *b = (Block *)p - 1;
    /* Keep the list sorted by address and merge neighbours. */
    Block **prev = &free_list;
    while (*prev && *prev < b) prev = &(*prev)->next;
    b->next = *prev;
    *prev = b;
    if (b->next && (char *)(b + 1) + b->size == (char *)b->next) {
        b->size += sizeof(Block) + b->next->size;
        b->next = b->next->next;
    }
    if (prev != &free_list) {
        Block *before = (Block *)((char *)prev - offsetof(Block, next));
        if ((char *)(before + 1) + before->size == (char *)b) {
            before->size += sizeof(Block) + b->size;
            before->next = b->next;
        }
    }
}

void *calloc(size_t n, size_t size) {
    void *p = malloc(n * size);
    if (p) memset(p, 0, n * size);
    return p;
}

void *realloc(void *p, size_t n) {
    if (!p) return malloc(n);
    Block *b = (Block *)p - 1;
    if (b->size >= n) return p;
    void *q = malloc(n);
    if (q) {
        memcpy(q, p, b->size);
        free(p);
    }
    return q;
}

/* ------------------------------------------------------------------ */
/* Strings */

void *memcpy(void *d, const void *s, size_t n) {
    char *dp = d;
    const char *sp = s;
    if ((((uintptr_t)dp | (uintptr_t)sp) & 3) == 0) {
        while (n >= 4) {
            *(uint32_t *)dp = *(const uint32_t *)sp;
            dp += 4;
            sp += 4;
            n -= 4;
        }
    }
    while (n--) *dp++ = *sp++;
    return d;
}

void *memmove(void *d, const void *s, size_t n) {
    char *dp = d;
    const char *sp = s;
    if (dp < sp) return memcpy(d, s, n);
    while (n--) dp[n] = sp[n];
    return d;
}

void *memset(void *d, int c, size_t n) {
    unsigned char *p = d;
    if (((uintptr_t)p & 3) == 0) {
        uint32_t w = (unsigned char)c * 0x01010101u;
        while (n >= 4) {
            *(uint32_t *)p = w;
            p += 4;
            n -= 4;
        }
    }
    while (n--) *p++ = (unsigned char)c;
    return d;
}

int memcmp(const void *a, const void *b, size_t n) {
    const unsigned char *x = a, *y = b;
    for (; n; n--, x++, y++)
        if (*x != *y) return *x - *y;
    return 0;
}

void *memchr(const void *s, int c, size_t n) {
    const unsigned char *p = s;
    for (; n; n--, p++)
        if (*p == (unsigned char)c) return (void *)p;
    return 0;
}

size_t strlen(const char *s) {
    const char *p = s;
    while (*p) p++;
    return p - s;
}

size_t strnlen(const char *s, size_t n) {
    size_t i = 0;
    while (i < n && s[i]) i++;
    return i;
}

char *strcpy(char *d, const char *s) {
    char *r = d;
    while ((*d++ = *s++));
    return r;
}

char *strncpy(char *d, const char *s, size_t n) {
    size_t i = 0;
    for (; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}

char *strcat(char *d, const char *s) {
    strcpy(d + strlen(d), s);
    return d;
}

char *strncat(char *d, const char *s, size_t n) {
    char *p = d + strlen(d);
    while (n-- && *s) *p++ = *s++;
    *p = 0;
    return d;
}

int strcmp(const char *a, const char *b) {
    while (*a && *a == *b) a++, b++;
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t n) {
    for (; n; n--, a++, b++) {
        if (*a != *b) return (unsigned char)*a - (unsigned char)*b;
        if (!*a) return 0;
    }
    return 0;
}

int strcasecmp(const char *a, const char *b) {
    while (*a && tolower(*a) == tolower(*b)) a++, b++;
    return tolower((unsigned char)*a) - tolower((unsigned char)*b);
}

int strncasecmp(const char *a, const char *b, size_t n) {
    for (; n; n--, a++, b++) {
        int x = tolower((unsigned char)*a), y = tolower((unsigned char)*b);
        if (x != y) return x - y;
        if (!x) return 0;
    }
    return 0;
}

char *strchr(const char *s, int c) {
    for (;; s++) {
        if (*s == (char)c) return (char *)s;
        if (!*s) return 0;
    }
}

char *strrchr(const char *s, int c) {
    const char *r = 0;
    for (;; s++) {
        if (*s == (char)c) r = s;
        if (!*s) return (char *)r;
    }
}

char *strstr(const char *h, const char *n) {
    size_t len = strlen(n);
    for (; *h; h++)
        if (!strncmp(h, n, len)) return (char *)h;
    return len ? 0 : (char *)h;
}

char *strdup(const char *s) {
    char *p = malloc(strlen(s) + 1);
    if (p) strcpy(p, s);
    return p;
}

char *strerror(int e) {
    switch (e) {
    case ENOENT: return "dosya yok";
    case ENOMEM: return "bellek yetmedi";
    default: return "hata";
    }
}

/* ------------------------------------------------------------------ */
/* Numbers */

int abs(int x) { return x < 0 ? -x : x; }
long labs(long x) { return x < 0 ? -x : x; }

long strtol(const char *s, char **end, int base) {
    while (isspace((unsigned char)*s)) s++;
    int neg = 0;
    if (*s == '-' || *s == '+') neg = *s++ == '-';
    unsigned long v = strtoul(s, end, base);
    return neg ? -(long)v : (long)v;
}

unsigned long strtoul(const char *s, char **end, int base) {
    while (isspace((unsigned char)*s)) s++;
    if ((base == 0 || base == 16) && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) {
        s += 2;
        base = 16;
    } else if (base == 0 && s[0] == '0') {
        base = 8;
    } else if (base == 0) {
        base = 10;
    }
    unsigned long v = 0;
    for (;; s++) {
        int d;
        if (isdigit((unsigned char)*s)) d = *s - '0';
        else if (isalpha((unsigned char)*s)) d = tolower((unsigned char)*s) - 'a' + 10;
        else break;
        if (d >= base) break;
        v = v * base + d;
    }
    if (end) *end = (char *)s;
    return v;
}

int atoi(const char *s) { return (int)strtol(s, 0, 10); }
long atol(const char *s) { return strtol(s, 0, 10); }

double strtod(const char *s, char **end) {
    while (isspace((unsigned char)*s)) s++;
    double sign = 1, v = 0;
    if (*s == '-' || *s == '+') sign = *s++ == '-' ? -1 : 1;
    while (isdigit((unsigned char)*s)) v = v * 10 + (*s++ - '0');
    if (*s == '.') {
        double f = 0.1;
        for (s++; isdigit((unsigned char)*s); s++, f /= 10) v += (*s - '0') * f;
    }
    if (end) *end = (char *)s;
    return sign * v;
}

double atof(const char *s) { return strtod(s, 0); }

static uint32_t rand_state = 1;
int rand(void) {
    rand_state = rand_state * 1103515245u + 12345u;
    return (rand_state >> 1) & RAND_MAX;
}
void srand(unsigned seed) { rand_state = seed; }

char *getenv(const char *name) { (void)name; return 0; }
int system(const char *cmd) { (void)cmd; return -1; }

void qsort(void *base, size_t n, size_t size, int (*cmp)(const void *, const void *)) {
    /* Insertion sort: simple, and n is small in practice. */
    char *b = base;
    char tmp[64];
    for (size_t i = 1; i < n; i++) {
        for (size_t j = i; j > 0 && cmp(b + (j - 1) * size, b + j * size) > 0; j--) {
            char *x = b + (j - 1) * size, *y = b + j * size;
            for (size_t k = 0; k < size; k += sizeof tmp) {
                size_t m = size - k < sizeof tmp ? size - k : sizeof tmp;
                memcpy(tmp, x + k, m);
                memcpy(x + k, y + k, m);
                memcpy(y + k, tmp, m);
            }
        }
    }
}

double fabs(double x) { return x < 0 ? -x : x; }
double floor(double x) {
    double t = (double)(long long)x;
    return t > x ? t - 1 : t;
}
double ceil(double x) {
    double t = (double)(long long)x;
    return t < x ? t + 1 : t;
}
double sqrt(double x) {
    if (x <= 0) return 0;
    double r = x > 1 ? x : 1;
    for (int i = 0; i < 40; i++) r = 0.5 * (r + x / r);
    return r;
}
double sin(double x) {
    while (x > M_PI) x -= 2 * M_PI;
    while (x < -M_PI) x += 2 * M_PI;
    double x2 = x * x, term = x, sum = x;
    for (int n = 1; n < 10; n++) {
        term *= -x2 / ((2 * n) * (2 * n + 1));
        sum += term;
    }
    return sum;
}
double cos(double x) { return sin(x + M_PI / 2); }
double tan(double x) { return sin(x) / cos(x); }
double atan(double x) {
    if (x > 1) return M_PI / 2 - atan(1 / x);
    if (x < -1) return -M_PI / 2 - atan(1 / x);
    double x2 = x * x, term = x, sum = x;
    for (int n = 1; n < 40; n++) {
        term *= -x2;
        sum += term / (2 * n + 1);
    }
    return sum;
}
double pow(double x, double y) {
    int n = (int)y;
    double r = 1;
    for (int i = 0; i < (n < 0 ? -n : n); i++) r *= x;
    return n < 0 ? 1 / r : r;
}

/* ------------------------------------------------------------------ */
/* Program end */

void exit(int code) {
    printf("\n[program %d koduyla bitti]\n", code);
    __asm__ volatile("ebreak");
    for (;;);
}

void abort(void) {
    puts("\n[abort]");
    __asm__ volatile("ebreak");
    for (;;);
}

void __assert_fail(const char *expr, const char *file, int line) {
    printf("%s:%d: assert başarısız: %s\n", file, line, expr);
    abort();
}

/* ------------------------------------------------------------------ */
/* Time */

int usleep(unsigned usec) {
    uint32_t until = mk_millis() + usec / 1000;
    while ((int32_t)(mk_millis() - until) < 0);
    return 0;
}

int gettimeofday(struct timeval *tv, void *tz) {
    (void)tz;
    uint32_t ms = mk_millis();
    tv->tv_sec = ms / 1000;
    tv->tv_usec = (ms % 1000) * 1000;
    return 0;
}

time_t time(time_t *t) {
    time_t v = mk_millis() / 1000;
    if (t) *t = v;
    return v;
}

int isatty(int fd) { return fd < 3; }

/* ------------------------------------------------------------------ */
/* Files */

#define MAX_FILES 32
#define FILE_STD 1

typedef struct {
    char name[64];
    unsigned char *data;
    size_t size, cap;
    int used, writable;
} Node;

struct FILE {
    Node *node;
    size_t pos;
    int flags, eof, open;
};

static Node nodes[MAX_FILES];
static FILE files[MAX_FILES];
static FILE std_files[3] = {{0, 0, FILE_STD, 0, 1}, {0, 0, FILE_STD, 0, 1}, {0, 0, FILE_STD, 0, 1}};
FILE *stdin = &std_files[0], *stdout = &std_files[1], *stderr = &std_files[2];

/* Strip directories: the file system is flat. */
static const char *base_name(const char *path) {
    const char *s = strrchr(path, '/');
    return s ? s + 1 : path;
}

static Node *find_node(const char *path) {
    const char *n = base_name(path);
    for (int i = 0; i < MAX_FILES; i++)
        if (nodes[i].used && !strcasecmp(nodes[i].name, n)) return &nodes[i];
    return 0;
}

static Node *new_node(const char *path) {
    for (int i = 0; i < MAX_FILES; i++) {
        if (!nodes[i].used) {
            Node *n = &nodes[i];
            memset(n, 0, sizeof *n);
            strncpy(n->name, base_name(path), sizeof n->name - 1);
            n->used = 1;
            return n;
        }
    }
    return 0;
}

void mk_add_file(const char *name, const void *data, size_t size) {
    Node *n = new_node(name);
    if (n) {
        n->data = (unsigned char *)data;
        n->size = n->cap = size;
    }
}

FILE *fopen(const char *path, const char *mode) {
    int write = strchr(mode, 'w') || strchr(mode, 'a');
    Node *n = find_node(path);
    if (!n) {
        if (!write) {
            errno = ENOENT;
            return 0;
        }
        n = new_node(path);
        if (!n) {
            errno = ENOMEM;
            return 0;
        }
        n->writable = 1;
    }
    if (write && !n->writable) {
        /* Copy an embedded file before writing to it. */
        unsigned char *copy = malloc(n->size ? n->size : 1);
        if (!copy) return 0;
        memcpy(copy, n->data, n->size);
        n->data = copy;
        n->cap = n->size;
        n->writable = 1;
    }
    if (strchr(mode, 'w')) n->size = 0;
    for (int i = 0; i < MAX_FILES; i++) {
        if (!files[i].open) {
            FILE *f = &files[i];
            f->node = n;
            f->pos = strchr(mode, 'a') ? n->size : 0;
            f->eof = 0;
            f->open = 1;
            f->flags = 0;
            return f;
        }
    }
    return 0;
}

int fclose(FILE *f) {
    if (f && !(f->flags & FILE_STD)) f->open = 0;
    return 0;
}

size_t fread(void *p, size_t size, size_t n, FILE *f) {
    if (!f || !f->node || !size) return 0;
    size_t want = size * n, have = f->node->size - f->pos;
    if (f->pos > f->node->size) have = 0;
    if (want > have) {
        want = have - have % size;
        f->eof = 1;
    }
    memcpy(p, f->node->data + f->pos, want);
    f->pos += want;
    return want / size;
}

static void console_write(const char *s, size_t n) {
    for (size_t i = 0; i < n; i++) MK_CONSOLE = (unsigned char)s[i];
}

size_t fwrite(const void *p, size_t size, size_t n, FILE *f) {
    size_t len = size * n;
    if (!f) return 0;
    if (f->flags & FILE_STD) {
        console_write(p, len);
        return n;
    }
    Node *nd = f->node;
    if (f->pos + len > nd->cap) {
        size_t cap = (f->pos + len) * 2;
        unsigned char *d = realloc(nd->cap ? nd->data : 0, cap);
        if (!d) return 0;
        nd->data = d;
        nd->cap = cap;
    }
    memcpy(nd->data + f->pos, p, len);
    f->pos += len;
    if (f->pos > nd->size) nd->size = f->pos;
    return n;
}

int fseek(FILE *f, long off, int whence) {
    if (!f || !f->node) return -1;
    long base = whence == SEEK_SET ? 0 : whence == SEEK_CUR ? (long)f->pos : (long)f->node->size;
    if (base + off < 0) return -1;
    f->pos = base + off;
    f->eof = 0;
    return 0;
}

long ftell(FILE *f) { return f && f->node ? (long)f->pos : -1; }
int feof(FILE *f) { return f && f->eof; }
int fflush(FILE *f) { (void)f; return 0; }
int fileno(FILE *f) { return f == stdin ? 0 : f == stdout ? 1 : f == stderr ? 2 : 3; }

int fgetc(FILE *f) {
    unsigned char c;
    return fread(&c, 1, 1, f) == 1 ? c : EOF;
}

char *fgets(char *s, int n, FILE *f) {
    int i = 0;
    while (i < n - 1) {
        int c = fgetc(f);
        if (c == EOF) break;
        s[i++] = c;
        if (c == '\n') break;
    }
    if (!i) return 0;
    s[i] = 0;
    return s;
}

int fputc(int c, FILE *f) {
    unsigned char b = c;
    return fwrite(&b, 1, 1, f) == 1 ? c : EOF;
}
int putc(int c, FILE *f) { return fputc(c, f); }
int putchar(int c) { return fputc(c, stdout); }
int fputs(const char *s, FILE *f) { return fwrite(s, 1, strlen(s), f) ? 0 : EOF; }
int puts(const char *s) {
    fputs(s, stdout);
    return putchar('\n');
}

int remove(const char *path) {
    Node *n = find_node(path);
    if (!n) return -1;
    n->used = 0;
    return 0;
}

int rename(const char *a, const char *b) {
    Node *n = find_node(a);
    if (!n) return -1;
    remove(b);
    strncpy(n->name, base_name(b), sizeof n->name - 1);
    return 0;
}

int stat(const char *path, struct stat *st) {
    Node *n = find_node(path);
    if (!n) return -1;
    st->st_size = n->size;
    st->st_mode = 0;
    return 0;
}

int access(const char *path, int mode) {
    (void)mode;
    return find_node(path) ? 0 : -1;
}

int mkdir(const char *path, mode_t mode) {
    (void)path;
    (void)mode;
    return 0;
}

void perror(const char *s) { printf("%s: %s\n", s, strerror(errno)); }

/* ------------------------------------------------------------------ */
/* Formatted output */

typedef struct {
    char *buf;
    size_t cap, len;
    FILE *file;
} Out;

static void out_char(Out *o, char c) {
    if (o->file) {
        fputc(c, o->file);
    } else if (o->len + 1 < o->cap) {
        o->buf[o->len] = c;
    }
    o->len++;
}

static void out_str(Out *o, const char *s, int width, int left, int prec) {
    int n = prec >= 0 ? (int)strnlen(s, prec) : (int)strlen(s);
    if (!left)
        for (int i = n; i < width; i++) out_char(o, ' ');
    for (int i = 0; i < n; i++) out_char(o, s[i]);
    if (left)
        for (int i = n; i < width; i++) out_char(o, ' ');
}

static void out_num(Out *o, unsigned long long v, int neg, int base, int upper, int width,
                    int left, int zero, int prec, int plus) {
    char tmp[32];
    int n = 0;
    const char *digits = upper ? "0123456789ABCDEF" : "0123456789abcdef";
    do {
        tmp[n++] = digits[v % base];
        v /= base;
    } while (v);
    while (n < prec) tmp[n++] = '0';
    char sign = neg ? '-' : plus ? '+' : 0;
    int total = n + (sign != 0);
    if (!left && !zero)
        for (int i = total; i < width; i++) out_char(o, ' ');
    if (sign) out_char(o, sign);
    if (!left && zero)
        for (int i = total; i < width; i++) out_char(o, '0');
    while (n) out_char(o, tmp[--n]);
    if (left)
        for (int i = total; i < width; i++) out_char(o, ' ');
}

static int format(Out *o, const char *fmt, va_list ap) {
    for (; *fmt; fmt++) {
        if (*fmt != '%') {
            out_char(o, *fmt);
            continue;
        }
        fmt++;
        int left = 0, zero = 0, plus = 0, width = 0, prec = -1, lng = 0;
        for (;; fmt++) {
            if (*fmt == '-') left = 1;
            else if (*fmt == '0') zero = 1;
            else if (*fmt == '+') plus = 1;
            else if (*fmt == ' ' || *fmt == '#') {}
            else break;
        }
        if (*fmt == '*') {
            width = va_arg(ap, int);
            fmt++;
        } else {
            while (isdigit((unsigned char)*fmt)) width = width * 10 + (*fmt++ - '0');
        }
        if (*fmt == '.') {
            fmt++;
            prec = 0;
            if (*fmt == '*') {
                prec = va_arg(ap, int);
                fmt++;
            } else {
                while (isdigit((unsigned char)*fmt)) prec = prec * 10 + (*fmt++ - '0');
            }
        }
        while (*fmt == 'l' || *fmt == 'h' || *fmt == 'z') {
            if (*fmt == 'l') lng++;
            fmt++;
        }
        switch (*fmt) {
        case 'd':
        case 'i': {
            long long v = lng >= 2 ? va_arg(ap, long long) : va_arg(ap, int);
            out_num(o, v < 0 ? -(unsigned long long)v : (unsigned long long)v, v < 0, 10, 0,
                    width, left, zero, prec, plus);
            break;
        }
        case 'u':
        case 'x':
        case 'X':
        case 'o': {
            unsigned long long v = lng >= 2 ? va_arg(ap, unsigned long long) : va_arg(ap, unsigned);
            int base = *fmt == 'u' ? 10 : *fmt == 'o' ? 8 : 16;
            out_num(o, v, 0, base, *fmt == 'X', width, left, zero, prec, 0);
            break;
        }
        case 'p':
            out_str(o, "0x", 0, 0, -1);
            out_num(o, (uintptr_t)va_arg(ap, void *), 0, 16, 0, 8, 0, 1, -1, 0);
            break;
        case 'c':
            out_char(o, (char)va_arg(ap, int));
            break;
        case 's': {
            const char *s = va_arg(ap, const char *);
            out_str(o, s ? s : "(null)", width, left, prec);
            break;
        }
        case 'f':
        case 'g': {
            double v = va_arg(ap, double);
            if (v < 0) {
                out_char(o, '-');
                v = -v;
            }
            if (prec < 0) prec = 6;
            unsigned long long ip = (unsigned long long)v;
            out_num(o, ip, 0, 10, 0, 0, 0, 0, -1, 0);
            if (prec) {
                out_char(o, '.');
                double f = v - ip;
                for (int i = 0; i < prec; i++) {
                    f *= 10;
                    int d = (int)f;
                    out_char(o, '0' + d);
                    f -= d;
                }
            }
            break;
        }
        case '%':
            out_char(o, '%');
            break;
        case 0:
            return o->len;
        default:
            out_char(o, '%');
            out_char(o, *fmt);
        }
    }
    return o->len;
}

int vsnprintf(char *buf, size_t n, const char *fmt, va_list ap) {
    Out o = {buf, n, 0, 0};
    int len = format(&o, fmt, ap);
    if (n) buf[o.len < n ? o.len : n - 1] = 0;
    return len;
}

int vsprintf(char *buf, const char *fmt, va_list ap) { return vsnprintf(buf, (size_t)-1 >> 1, fmt, ap); }

int vfprintf(FILE *f, const char *fmt, va_list ap) {
    Out o = {0, 0, 0, f};
    return format(&o, fmt, ap);
}

int vprintf(const char *fmt, va_list ap) { return vfprintf(stdout, fmt, ap); }

int printf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vprintf(fmt, ap);
    va_end(ap);
    return r;
}

int fprintf(FILE *f, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vfprintf(f, fmt, ap);
    va_end(ap);
    return r;
}

int sprintf(char *buf, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vsprintf(buf, fmt, ap);
    va_end(ap);
    return r;
}

int snprintf(char *buf, size_t n, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vsnprintf(buf, n, fmt, ap);
    va_end(ap);
    return r;
}

/* Only what Doom needs: integers and strings separated by literal text. */
int sscanf(const char *s, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int count = 0;
    while (*fmt) {
        if (isspace((unsigned char)*fmt)) {
            while (isspace((unsigned char)*s)) s++;
            fmt++;
            continue;
        }
        if (*fmt != '%') {
            if (*s != *fmt) break;
            s++, fmt++;
            continue;
        }
        fmt++;
        int width = 0;
        while (isdigit((unsigned char)*fmt)) width = width * 10 + (*fmt++ - '0');
        while (*fmt == 'l' || *fmt == 'h') fmt++;
        char *end;
        if (*fmt == 'd' || *fmt == 'i' || *fmt == 'x' || *fmt == 'u') {
            int base = *fmt == 'x' ? 16 : *fmt == 'i' ? 0 : 10;
            long v = strtol(s, &end, base);
            if (end == s) break;
            *va_arg(ap, int *) = (int)v;
            s = end;
        } else if (*fmt == 's') {
            while (isspace((unsigned char)*s)) s++;
            char *d = va_arg(ap, char *);
            int n = 0;
            while (*s && !isspace((unsigned char)*s) && (!width || n < width)) d[n++] = *s++;
            d[n] = 0;
            if (!n) break;
        } else if (*fmt == 'c') {
            if (!*s) break;
            *va_arg(ap, char *) = *s++;
        } else {
            break;
        }
        count++;
        fmt++;
    }
    va_end(ap);
    return count;
}
