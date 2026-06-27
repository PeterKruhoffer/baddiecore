# Baddiecore Local Database

Start the local MySQL database with:

```bash
docker compose up -d mysql
```

The ASP.NET Core development connection string targets `127.0.0.1:3307`.

The scripts in `db/init` create the CMS schema and idempotent demo content for local development. MySQL only runs init scripts when it creates a new data volume. To seed an existing local volume, run:

```bash
docker compose exec -T mysql mysql -ubaddiecore -pbaddiecore_dev_password baddiecore < db/init/002_demo_content.sql
```

The seed uses non-destructive upserts and resolves dependencies by natural key, so re-running it does not overwrite existing content or depend on particular database IDs.
