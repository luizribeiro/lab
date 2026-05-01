#include <os/log.h>

void lockin_observe_emit_public_log(const char *msg) {
    os_log(OS_LOG_DEFAULT, "%{public}s", msg);
}
