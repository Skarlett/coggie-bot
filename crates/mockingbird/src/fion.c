#include <fcntl.h>
#include <sys/ioctl.h>

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
