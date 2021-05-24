*** Assumptions

- «frozen» and «locked» are the same thing
- we don't need to support client's total > 2^64/1e4 ~ 1.8 quadrillon

*** Testing

I used quickcheck to generate random test cases and check that output amounts are correct. Also included modified transactions.csv
