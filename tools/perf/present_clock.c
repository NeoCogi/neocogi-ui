/* Measures complete frame intervals in one explicitly launched Vulkan process.
 * The clock observes presentation entry; it never changes descriptors or results.
 * Startup and shader compilation precede the configurable warmup boundary. */
#define _GNU_SOURCE
#include <vulkan/vulkan.h>
#include <dlfcn.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* One UI thread owns the application's presentation queue and these observations. */
static PFN_vkGetInstanceProcAddr real_instance_proc;
static PFN_vkGetDeviceProcAddr real_device_proc;
static PFN_vkQueuePresentKHR real_present;
static uint64_t presentations, first_ns, last_ns, failures;
static uint64_t warmup = 1000;

/* Wall time inside queue/target synchronization explains off-CPU desktop variance. */
struct Stage {
    /* Exact native function name reported alongside the aggregate. */
    const char *name;
    /* Calls observed after warmup, including final native cleanup. */
    uint64_t calls;
    /* Sum of monotonic wall durations; includes both CPU work and waiting. */
    uint64_t ns;
};
static struct Stage stages[] = {
    {"vkWaitForFences", 0, 0}, {"vkAcquireNextImageKHR", 0, 0},
    {"vkQueueSubmit", 0, 0}, {"vkQueuePresentKHR", 0, 0}, {"vkDeviceWaitIdle", 0, 0},
};

static uint64_t now_ns(void) {
    struct timespec value;
    clock_gettime(CLOCK_MONOTONIC_RAW, &value);
    return (uint64_t)value.tv_sec * UINT64_C(1000000000) + (uint64_t)value.tv_nsec;
}

static VkResult VKAPI_CALL observed_present(VkQueue queue, const VkPresentInfoKHR *info) {
    uint64_t timestamp = now_ns();
    if (presentations == warmup) first_ns = timestamp;
    last_ns = timestamp;
    presentations++;
    VkResult result = real_present(queue, info);
    if (presentations > warmup) { stages[3].calls++; stages[3].ns += now_ns() - timestamp; }
    if (result != VK_SUCCESS && result != VK_SUBOPTIMAL_KHR) failures++;
    return result;
}

#define OBSERVE_RESULT(command, slot, parameters, arguments) \
    static PFN_##command real_##command; \
    static VkResult VKAPI_CALL observed_##command parameters { \
        uint64_t start = now_ns(); \
        VkResult result = real_##command arguments; \
        if (presentations >= warmup) { stages[slot].calls++; stages[slot].ns += now_ns() - start; } \
        return result; \
    }
OBSERVE_RESULT(vkWaitForFences, 0,
    (VkDevice device, uint32_t count, const VkFence *fences, VkBool32 all, uint64_t timeout),
    (device, count, fences, all, timeout))
OBSERVE_RESULT(vkAcquireNextImageKHR, 1,
    (VkDevice device, VkSwapchainKHR swapchain, uint64_t timeout, VkSemaphore semaphore, VkFence fence, uint32_t *index),
    (device, swapchain, timeout, semaphore, fence, index))
OBSERVE_RESULT(vkQueueSubmit, 2,
    (VkQueue queue, uint32_t count, const VkSubmitInfo *submits, VkFence fence),
    (queue, count, submits, fence))
OBSERVE_RESULT(vkDeviceWaitIdle, 4, (VkDevice device), (device))

static PFN_vkVoidFunction observe_function(const char *name, PFN_vkVoidFunction function) {
    if (!function) return function;
    if (strcmp(name, "vkQueuePresentKHR") == 0) {
        real_present = (PFN_vkQueuePresentKHR)function;
        return (PFN_vkVoidFunction)observed_present;
    }
#define SELECT_OBSERVER(command) \
    if (strcmp(name, #command) == 0) { real_##command = (PFN_##command)function; return (PFN_vkVoidFunction)observed_##command; }
    SELECT_OBSERVER(vkWaitForFences)
    SELECT_OBSERVER(vkAcquireNextImageKHR)
    SELECT_OBSERVER(vkQueueSubmit)
    SELECT_OBSERVER(vkDeviceWaitIdle)
    return function;
}

/* ash loads device functions through these dispatch queries. Return the actual function for
 * every unobserved name, and preserve the real presentation function behind the wrapper. */
VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetDeviceProcAddr(VkDevice device, const char *name) {
    if (!real_device_proc) real_device_proc = (PFN_vkGetDeviceProcAddr)dlsym(RTLD_NEXT, "vkGetDeviceProcAddr");
    if (!real_device_proc) return NULL;
    PFN_vkVoidFunction function = real_device_proc(device, name);
    return observe_function(name, function);
}

VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance instance, const char *name) {
    if (!real_instance_proc) real_instance_proc = (PFN_vkGetInstanceProcAddr)dlsym(RTLD_NEXT, "vkGetInstanceProcAddr");
    if (!real_instance_proc) return NULL;
    PFN_vkVoidFunction function = real_instance_proc(instance, name);
    if (function && strcmp(name, "vkGetDeviceProcAddr") == 0) {
        real_device_proc = function == (PFN_vkVoidFunction)vkGetDeviceProcAddr
            ? (PFN_vkGetDeviceProcAddr)dlsym(RTLD_NEXT, "vkGetDeviceProcAddr")
            : (PFN_vkGetDeviceProcAddr)function;
        return (PFN_vkVoidFunction)vkGetDeviceProcAddr;
    }
    return observe_function(name, function);
}

__attribute__((constructor)) static void configure_clock(void) {
    const char *value = getenv("NEOCOGI_PROFILE_WARMUP");
    if (value) warmup = strtoull(value, NULL, 10);
}

__attribute__((destructor)) static void report_clock(void) {
    if (!first_ns || presentations <= warmup + 1 || last_ns <= first_ns) return;
    uint64_t intervals = presentations - warmup - 1;
    double seconds = (double)(last_ns - first_ns) / 1e9;
    fprintf(stderr, "NEOCOGI_PRESENT_CLOCK {\"presentations\":%" PRIu64
        ",\"warmup\":%" PRIu64 ",\"intervals\":%" PRIu64
        ",\"seconds\":%.9f,\"fps\":%.3f,\"failures\":%" PRIu64 "}\n",
        presentations, warmup, intervals, seconds, (double)intervals / seconds, failures);
    for (size_t index = 0; index < sizeof(stages) / sizeof(stages[0]); index++) {
        fprintf(stderr, "NEOCOGI_VULKAN_STAGE {\"name\":\"%s\",\"calls\":%" PRIu64 ",\"seconds\":%.9f}\n",
            stages[index].name, stages[index].calls, (double)stages[index].ns / 1e9);
    }
}
