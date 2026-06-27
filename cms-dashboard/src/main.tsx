import { render } from "solid-js/web";

const isPreview = window.location.pathname.replace(/\/$/, "").endsWith("/cms/preview");
const appModule = isPreview ? await import("./PreviewApp") : await import("./App");
const Root = appModule.default;

render(() => <Root />, document.getElementById("root") as HTMLElement);
