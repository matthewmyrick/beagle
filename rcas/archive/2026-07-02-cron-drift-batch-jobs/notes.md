# Notes

```text
$ chronyc tracking
Reference ID    : 00000000 ()
Stratum         : 0
System time     : 2831.4 seconds fast of NTP time
Leap status     : Not synchronised
```

- Lock file: /run/chrony/chronyd.pid, mtime = reboot time 06-28
- Window-check rejections in scheduler log: 41 jobs, all "outside
  execution window"
