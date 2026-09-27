import { render } from "@solidjs/web";import "./styles.css";import { AdminApp } from "./admin/AdminApp";import { PublicPage } from "./PublicPage";
render(()=>location.pathname.startsWith("/admin")?<AdminApp/>:<PublicPage/>,document.getElementById("root")!);
