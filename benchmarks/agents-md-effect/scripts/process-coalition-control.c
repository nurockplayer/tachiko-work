#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PROC_PIDCOALITIONINFO 20
#define COALITION_TYPE_RESOURCE 0
#define COALITION_TYPE_JETSAM 1
#define COALITION_NUM_TYPES 2

struct tachiko_proc_pidcoalitioninfo {
    uint64_t coalition_id[COALITION_NUM_TYPES];
    uint64_t reserved1;
    uint64_t reserved2;
    uint64_t reserved3;
};

static int coalition_info(pid_t pid, struct tachiko_proc_pidcoalitioninfo *info) {
    memset(info, 0, sizeof(*info));
    return proc_pidinfo(pid, PROC_PIDCOALITIONINFO, 0, info, (int)sizeof(*info));
}

static uint64_t parse_u64(const char *value, const char *label) {
    char *end = NULL;
    errno = 0;
    uint64_t result = strtoull(value, &end, 10);
    if (errno != 0 || end == value || *end != '\0' || result == 0) {
        fprintf(stderr, "invalid %s\n", label);
        exit(64);
    }
    return result;
}

static pid_t parse_pid(const char *value) {
    uint64_t parsed = parse_u64(value, "pid");
    if (parsed > INT32_MAX) {
        fprintf(stderr, "pid out of range\n");
        exit(64);
    }
    return (pid_t)parsed;
}

static int compare_pid(const void *left, const void *right) {
    const pid_t a = *(const pid_t *)left;
    const pid_t b = *(const pid_t *)right;
    return (a > b) - (a < b);
}

struct scan_receipt {
    int attempts;
    int initial_count_hint;
    int capacities[8];
    int counts[8];
    int invalid_pid_entries[8];
    int duplicate_pid_entries[8];
};

static int coalition_members(uint64_t resource_id, pid_t **members_out,
                             struct scan_receipt *receipt) {
    int count_hint = proc_listallpids(NULL, 0);
    if (count_hint <= 0) {
        perror("proc_listallpids(size)");
        exit(70);
    }
    size_t capacity = (size_t)count_hint + 256;
    pid_t *all = NULL;
    int count = 0;
    int attempts = 0;
    int complete_scan_streak = 0;
    for (;;) {
        if (capacity > (size_t)INT32_MAX / sizeof(pid_t)) {
            fprintf(stderr, "PID scan capacity exceeds API bounds\n");
            exit(70);
        }
        free(all);
        all = calloc(capacity, sizeof(pid_t));
        if (all == NULL) {
            perror("calloc");
            exit(70);
        }
        attempts++;
        count = proc_listallpids(all, (int)(capacity * sizeof(pid_t)));
        if (count < 0) {
            perror("proc_listallpids");
            exit(70);
        }
        receipt->capacities[attempts - 1] = (int)capacity;
        receipt->counts[attempts - 1] = count;
        qsort(all, (size_t)count, sizeof(pid_t), compare_pid);
        int invalid_pid_entries = 0;
        int duplicate_pid_entries = 0;
        for (int index = 0; index < count; index++) {
            if (all[index] <= 0) invalid_pid_entries++;
            if (all[index] > 0 && index > 0 && all[index - 1] == all[index]) {
                duplicate_pid_entries++;
            }
        }
        receipt->invalid_pid_entries[attempts - 1] = invalid_pid_entries;
        receipt->duplicate_pid_entries[attempts - 1] = duplicate_pid_entries;
        if ((size_t)count < capacity && duplicate_pid_entries == 0) {
            complete_scan_streak++;
            if (complete_scan_streak >= 2) break;
        } else {
            complete_scan_streak = 0;
        }
        if (attempts >= 8) {
            fprintf(stderr, "PID scan did not reach a complete valid snapshot\n");
            exit(70);
        }
        if ((size_t)count >= capacity) capacity *= 2;
    }
    pid_t *members = calloc(capacity, sizeof(pid_t));
    if (members == NULL) {
        perror("calloc");
        exit(70);
    }
    int member_count = 0;
    for (int index = 0; index < count; index++) {
        if (all[index] <= 0 || (index > 0 && all[index - 1] == all[index])) continue;
        struct tachiko_proc_pidcoalitioninfo info;
        if (coalition_info(all[index], &info) == (int)sizeof(info) &&
            info.coalition_id[COALITION_TYPE_RESOURCE] == resource_id) {
            members[member_count++] = all[index];
        }
    }
    free(all);
    qsort(members, (size_t)member_count, sizeof(pid_t), compare_pid);
    for (int index = 1; index < member_count; index++) {
        if (members[index - 1] == members[index]) {
            fprintf(stderr, "PID scan returned duplicate coalition members\n");
            exit(70);
        }
    }
    receipt->attempts = attempts;
    receipt->initial_count_hint = count_hint;
    *members_out = members;
    return member_count;
}

int main(int argc, char **argv) {
    if (argc == 3 && strcmp(argv[1], "info") == 0) {
        pid_t pid = parse_pid(argv[2]);
        struct tachiko_proc_pidcoalitioninfo info;
        if (coalition_info(pid, &info) != (int)sizeof(info)) {
            perror("proc_pidinfo(PROC_PIDCOALITIONINFO)");
            return 69;
        }
        printf("{\"pid\":%d,\"resource_coalition_id\":\"%" PRIu64
               "\",\"jetsam_coalition_id\":\"%" PRIu64 "\"}\n",
               pid, info.coalition_id[COALITION_TYPE_RESOURCE],
               info.coalition_id[COALITION_TYPE_JETSAM]);
        return 0;
    }
    if (argc == 3 && strcmp(argv[1], "members") == 0) {
        uint64_t resource_id = parse_u64(argv[2], "resource coalition id");
        pid_t *members = NULL;
        struct scan_receipt receipt;
        int count = coalition_members(resource_id, &members, &receipt);
        printf("{");
        printf("\"resource_coalition_id\":\"%" PRIu64
               "\",\"pid_list_complete\":true,\"scan_attempts\":%d,"
               "\"stable_complete_scans\":2,\"initial_count_hint\":%d,\"scans\":[", resource_id,
               receipt.attempts, receipt.initial_count_hint);
        for (int index = 0; index < receipt.attempts; index++) {
            if (index > 0) printf(",");
            printf("{\"capacity\":%d,\"count\":%d,\"invalid_pid_entries\":%d,"
                   "\"duplicate_pid_entries\":%d}", receipt.capacities[index],
                   receipt.counts[index], receipt.invalid_pid_entries[index],
                   receipt.duplicate_pid_entries[index]);
        }
        printf("],\"pids\":[");
        for (int index = 0; index < count; index++) {
            if (index > 0) printf(",");
            printf("%d", members[index]);
        }
        printf("]}\n");
        free(members);
        return 0;
    }
    fprintf(stderr, "usage: process-coalition-control info PID | members RESOURCE_COALITION_ID\n");
    return 64;
}
