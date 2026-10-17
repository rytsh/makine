#ifndef _SYS_STAT_H
#define _SYS_STAT_H
#include <sys/types.h>
struct stat { off_t st_size; mode_t st_mode; };
int stat(const char *path, struct stat *st);
int mkdir(const char *path, mode_t mode);
#define S_ISDIR(m) (0)
#endif
