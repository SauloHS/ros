#ifndef LIBROS_H
#define LIBROS_H

int read(int fd, char *buf, int len);
int write(int fd, const char *buf, int len);
void exit(int code);
int puts(const char *s);
int printf(const char *fmt, ...);

#endif
