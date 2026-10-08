// Generates a typed @solana/kit client for the program from the Anchor IDL.
// Run `anchor build` first, then `npm run generate-client`.
import { readFileSync } from "node:fs";
import { createFromRoot } from "codama";
import { rootNodeFromAnchor } from "@codama/nodes-from-anchor";
import { renderVisitor } from "@codama/renderers-js";

const idl = JSON.parse(readFileSync(new URL("../target/idl/otc_desk.json", import.meta.url), "utf8"));
const codama = createFromRoot(rootNodeFromAnchor(idl));

await codama.accept(
  renderVisitor(new URL("../app", import.meta.url).pathname, {
    generatedFolder: "src/generated",
    syncPackageJson: false,
    kitImportStrategy: "rootOnly",
  }),
);
console.log("Generated app/src/generated");
