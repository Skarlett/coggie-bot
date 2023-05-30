#define _GNU_SOURCE
#include <fcntl.h>
#include <sys/ioctl.h>
#include <unistd.h>

int availbytes(int fd)
{
    int bytes_available = 0;

    if (!fd) {
        return -1;
    }

    if (ioctl(fd, FIONREAD, &bytes_available) == -1) {
        return -2;
    }

    return bytes_available;
}

int bigpipe(int fd, int size) {
    // Get the current pipe size limit
    int current_limit = 0;
    
    current_limit = fcntl(fd, F_GETPIPE_SZ);
    if (current_limit > size) {
        return 0;
    }

    if (fcntl(fd, F_SETPIPE_SZ, size) != 0) {
        return -1;
    }
    current_limit = fcntl(fd, F_GETPIPE_SZ);
    
    if (current_limit != size) {
        return -2;
    }

    return 0;
}