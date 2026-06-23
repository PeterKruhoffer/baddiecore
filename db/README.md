# Baddiecore Local Database

Start the local MySQL database with:

```bash
docker compose up -d mysql
```

The ASP.NET Core development connection string targets `127.0.0.1:3307`.

The scripts in `db/init` create an empty CMS schema for local development. They do not seed pages, templates, renderings, or workflow data.
