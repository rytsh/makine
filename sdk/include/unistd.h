#ifndef _UNISTD_H
#define _UNISTD_H
#include <sys/types.h>
int usleep(unsigned usec);
int access(const char *path, int mode);
int isatty(int fd);
#define F_OK 0
#define R_OK 4
#endif
