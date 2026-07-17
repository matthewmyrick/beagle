# Impact

- **Duration:** 47 minutes (14:32–15:19 UTC)
- **Requests:** ~312k checkout requests degraded; 41,800 returned 504
- **Users:** est. 9,200 distinct users saw a failed or hung checkout
- **Money:** ~$61k GMV delayed; $4.1k in abandoned carts not recovered
  within 24h (finance estimate)
- **SLO:** burned 38% of the quarterly `payments-api` availability
  error budget

```text
metric              before      during      after
p99 latency         182 ms      7.4 s       176 ms
error rate          0.02 %      11.3 %      0.01 %
pool waiters        0           max (8)     0
```
