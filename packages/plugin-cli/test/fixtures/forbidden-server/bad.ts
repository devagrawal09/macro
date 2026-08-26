import fs from "node:fs";
export async function bad() {
	fs.readFileSync("secret");
}
