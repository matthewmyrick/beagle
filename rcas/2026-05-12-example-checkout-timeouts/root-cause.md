# What happened

1. Checkout requests began timing out under normal peak traffic.
2. Requests were waiting on a database connection that never became free.
3. The connection pool had been reduced to a size below what peak
   checkout traffic requires.
4. A configuration rollout intended for a lower-traffic environment was
   applied to production checkout.

The change passed automated checks because the value was valid in
isolation — the checks did not compare it against expected traffic.
