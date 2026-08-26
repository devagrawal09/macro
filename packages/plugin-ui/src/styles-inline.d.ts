/** Vite `?inline` CSS imports resolve to a string of stylesheet text. */
declare module "*.css?inline" {
	const css: string;
	export default css;
}
