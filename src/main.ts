import { mount } from "svelte";
import App from "./app/App.svelte";
import "./styles/tokens.css";
import "./styles/global.css";

const target = document.getElementById("app");
if (!target) {
  throw new Error("SpringCat: #app mount point is missing");
}

mount(App, { target });
