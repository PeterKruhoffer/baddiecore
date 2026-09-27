import { defineRailway, mysql, project, service } from "railway/iac";

export default defineRailway((ctx) => {
  const db = mysql("MySQL");
  const cms = service("baddiecore", {
    // Deploy this checkout with `railway up --service baddiecore`.
    // Railway detects the root Dockerfile.
    healthcheck: "/health",
    healthcheckTimeout: 120,
    replicas: 1,
    env: {
      DATABASE_URL: db.env.MYSQL_URL,
      BADDIE_ADMIN_PASSWORD: ctx.shared.BADDIE_ADMIN_PASSWORD,
      BADDIE_SECURE_COOKIE: "true",
    },
  });

  return project(ctx.projectName, { resources: [db, cms] });
});
