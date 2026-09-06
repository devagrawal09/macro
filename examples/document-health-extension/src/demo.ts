import { Macro, type MacroOpts } from "@macro/sdk";

const token = process.env.MACRO_API_KEY;
if (!token) throw new Error("MACRO_API_KEY is required");

const value = process.env.MACRO_ENV ?? "local";
if (value !== "local" && value !== "dev" && value !== "prod") {
	throw new Error(`Invalid MACRO_ENV: ${value}`);
}
const env = value as NonNullable<MacroOpts["env"]>;
const macro = new Macro({ env, token });
const document = await macro.documents.create({
	name: "Document Health demo",
	markdown: `# Launch brief

This draft is ready for review.

- [ ] Confirm the launch date
- [ ] Add customer evidence
`,
});

console.log(`Created ${document.id}`);
console.log(document.webUrl());
