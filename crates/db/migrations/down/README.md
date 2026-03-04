# Manual Down Migrations

These scripts are manual rollback scripts for migrations that need explicit down paths.

- `20260225000001_deployment_environments.down.sql` rolls back deployment environment schema introduced in `20260225000001_deployment_environments.sql`.

Use with caution in non-production first, for example:

```bash
psql "$DATABASE_URL" -f crates/db/migrations/down/20260225000001_deployment_environments.down.sql
```
