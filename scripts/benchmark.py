#!/usr/bin/env python3
"""Measure a running process without instrumenting it. macOS / libproc only."""
import argparse
import ctypes as C
import json
import platform
import time

class Usage(C.Structure):
    _fields_ = [("uuid", C.c_uint8 * 16)] + [(name, C.c_uint64) for name in (
        "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups", "pageins",
        "wired_size", "resident_size", "phys_footprint", "proc_start_abstime", "proc_exit_abstime",
        "child_user_time", "child_system_time", "child_pkg_idle_wkups", "child_interrupt_wkups",
        "child_pageins", "child_elapsed_abstime", "diskio_bytesread", "diskio_byteswritten")]

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("pid", type=int)
parser.add_argument("--seconds", type=int, default=60)
args = parser.parse_args()
if args.seconds < 1:
    parser.error("--seconds must be positive")
lib = C.CDLL("/usr/lib/libproc.dylib", use_errno=True)
lib.proc_pid_rusage.argtypes = [C.c_int, C.c_int, C.POINTER(Usage)]
lib.proc_pid_rusage.restype = C.c_int

def read():
    usage = Usage()
    if lib.proc_pid_rusage(args.pid, 2, C.byref(usage)):
        raise OSError(C.get_errno(), "proc_pid_rusage failed")
    return usage

first = read()
start = time.monotonic()
peak_rss, peak_footprint = first.resident_size, first.phys_footprint
while True:
    remaining = args.seconds - (time.monotonic() - start)
    if remaining <= 0:
        break
    time.sleep(min(5, remaining))
    current = read()
    peak_rss = max(peak_rss, current.resident_size)
    peak_footprint = max(peak_footprint, current.phys_footprint)
last = read()
elapsed = time.monotonic() - start
cpu = (last.user_time + last.system_time - first.user_time - first.system_time) / 1e9
print(json.dumps({
    "pid": args.pid, "macos": platform.mac_ver()[0], "architecture": platform.machine(),
    "seconds": round(elapsed, 2), "cpu_percent_one_core": round(cpu / elapsed * 100, 4),
    "physical_footprint_mib": round(last.phys_footprint / 2**20, 2),
    "peak_physical_footprint_mib": round(peak_footprint / 2**20, 2),
    "rss_mib": round(last.resident_size / 2**20, 2), "peak_rss_mib": round(peak_rss / 2**20, 2),
    "idle_wakeups_per_second": round((last.pkg_idle_wkups - first.pkg_idle_wkups) / elapsed, 3),
    "disk_bytes_read": last.diskio_bytesread - first.diskio_bytesread,
    "disk_bytes_written": last.diskio_byteswritten - first.diskio_byteswritten,
}, indent=2))
