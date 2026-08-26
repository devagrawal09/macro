// Task-inbox-style rename handler fake (prebuilt ESM bundle shape).
export async function handle(event, context) {
	context.log.info("renaming task", { taskId: event.taskId });
	if (!context.macro) throw new Error("expected an injected macro facade");
	await context.macro.rename(event.taskId, event.name, {
		signal: context.signal,
	});
	return { projectId: context.projectId, done: true };
}
