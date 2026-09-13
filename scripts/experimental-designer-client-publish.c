#define _GNU_SOURCE

/*
 * Atomically publish one completed experimental client-kit directory without
 * replacing any existing destination. This intentionally small helper is used
 * only by package-experimental-designer-client.sh after all kit files exist.
 */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>

#if defined(__APPLE__)
#include <sys/attr.h>
#include <unistd.h>
#elif defined(__linux__)
#include <sys/syscall.h>
#include <unistd.h>
#ifndef RENAME_NOREPLACE
#define RENAME_NOREPLACE (1U << 0)
#endif
#else
#error "experimental client-kit publication requires a platform no-replace rename"
#endif

int main(int argc, char **argv) {
  if (argc != 3) {
    fprintf(stderr, "usage: %s STAGED_DIRECTORY ABSENT_DESTINATION\n", argv[0]);
    return 64;
  }

#if defined(__APPLE__)
  if (renameatx_np(AT_FDCWD, argv[1], AT_FDCWD, argv[2], RENAME_EXCL) == 0) {
    return 0;
  }
#elif defined(__linux__)
  if (syscall(SYS_renameat2, AT_FDCWD, argv[1], AT_FDCWD, argv[2], RENAME_NOREPLACE) == 0) {
    return 0;
  }
#endif

  const int error = errno;
  perror("experimental client-kit publication");
  return error == EEXIST ? 17 : 1;
}
