# Summary

For 34 minutes on 2026-05-12, roughly 8% of checkout attempts timed out
after a configuration rollout narrowed a connection pool below what peak
traffic needed. We rolled the change back and checkout recovered fully;
no orders were lost, and delayed attempts succeeded on retry.
