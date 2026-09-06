Plugins are ways to extend the functionality of Macro with user generated code.

There are two types of plugins supported by Macro - client plugins and server plugins.

Server plugins extend the server side functionality by reacting to events inside a macro workspace and performing automated work using the macro sdk. Server plugins run in a secure sandbox (like cloudflare worker isolates) and similarly use dynamic limited scope credentials.

Client plugins extend the user interface of Macro using custom HTML views. Client plugins can also use the macro sdk to query data, perform actions, and subscribe to events for realtime updates.
Client plugins render inside an isolated browser sandbox without any access to the rest of the macro frontend or other plugins. It communicates with macro sdk using dynamically generated credentials with limited scope required for the plugin to work.
There are further two types of client plugins - page and sidebar. a page plugin renders its UI in an entire macro window next to the left navigation menu, and adds a link to the view in the navigation. a sidebar plugin appears as a sidebar item in an existing entity page (like task or customer) and gets injected context about where its rendered.

the macro sdk has operations to deploy plugins. developer could call the deploy plugin api with the raw html string for a client side plugin and a raw javascript function as a string for the server side plugin.

We also have a macro plugin toolkit, which is a small, opinionated, and typesafe tool to help developers and agents build, deploy, and monitor macro plugins.

The plugin toolkit can deploy plugins from a single typescript file that calls the definePlugin function from macro. The code for the client and server plugins are passed into defineplugin as direct callbacks. the cli takes this file and builds independent tree shaken bundles for each lcient and server plugin and deploys them to the macro workspace. the cli can also list, search, view execution logs, disable/enable, and remove plugins from the workspace.

additionally the macro settings UI provides a dashboard to see all the deployed plugins, their activity, etc.

client side plugins are objects with a name property, a scope property that defines authorization scopes, and a render function that is a solidjs component. this component receives the macro sdk as well as some other context and returns reactive UI. this component is rendered automatically by macro within the sandbox.

Why solid? client plugins are technically plain HTML files with ordinary javascript logic. theoretically we can provide developers the ability to dpeloy any html file that uses javascript and macro sdk to populate the UI, and let them bring their own framework like React. Since the plugin is isolated from macro's frontend, it doesn't need to use Solid.
however authoring UIs in solid is obviously much nicer than in vanilla js, definitely a bit nicer than react, and overall a good way to not worry about performance and just ship plugins.
Plus sharing a framework across plugins and core frontend also allows us to share UI components. We can consolidate and export a core set of UI components used within macro itself as a ui library to use when building macro plugins, which will allow custom user built plugins to potentially match macro's brand and theme and visual language. they could also share some sort of theme configuration. the user can ignore this library and build a completely custom ui if they want, but most users (and their agents) are actually gonna find the ui library very helpful to quickly author pluins that look like natural extensions of the product they are using.
Plus the toolkit takes care of building the standalone html file to deploy into macro from the plugin component automatically using vite, so developers dont have to set up the build toolchain themselves (even though it has gotten ridiculously easy to do that thanks to vite)

server side plugins are objects with a name, event to listen for, scopes to grant to the plugin, and a handle function that receives the event, macro sdk, and other context. the function cannot cause any side effects other than those through the macro sdk. it doesnt have filesystem or network access.

defineplugin can make the plugins typesafe by narrowing the provided macro sdk to only have operations authorized by the scope. if the scope only grants macro tasks access, the macro sdk injected into the plugin would only have the types for the tasks operations, and any other operation would not only fail at runtime but also be rejected at compile time.
server side plugins would see the type of the event subscribed to.
client sidebar plugins would also see typed context depending on which entity they will be attached to.
where all these would normally be untyped html/js logic or manually provided/narrowed types, the plugin toolkit provides a single fully typesafe and seamless experience to author and deploy plugins.

during deployment if the workspace already has a deployed plugin with the same name the cli will reject and make the user --overwrite/--force, or rename the conflitcing plugins in the js file. the name and path of the js file doesnt really matter, neither does the path you are executing the cli from. except for env vars and .env files, which the cli could automatically look up. thats where the macro credentials to deploy should be, along with the id of the workspace. so outside of using .env files in the directory you are executing the cli in and maybe its parents.

but outside of the env credentials and the options that go into defineplugin, there is no other configuraion required to build plugins for any macro workspace.

this plugin architecture allows nearly endless interface and workflow customization on top of macro's data model and integrations. but can those also be pluggable?

macro can support databases like notion/obsidian, but very well integrated into the macro suite, and with a nice datatable ui in the app. the macro sdk would expose crud operations and change events so plugins can not only customize on top of the existing macro data model, but invent their own, but instead of hidden custom state or events, the data model of these plugins is fully inspectable and connectable throughout the workspace. both client and server side plugins would interact with each other in realtime through this data model with macro essentially acting like a sync engine between them.

not one with local/offline data though. client plugins query or subscribe to live data, but are responsible for their own optimistic updates and offline availability. it's possible that eventually macro's own sync infrastructure could be shared with client plugins as well as the component library. at the same time, databases would allow even more macro built in features to move out of the core and convert to database+plugin setups (templates?)

finally, custom integrations. we talked a bit about how building a granola integration would be incredibly helkpful for a lot of people but that can probably never be a core built in feature, because then macro would just be an enormous pile of integrations. but custom integrations can be tricky.

when designing server plugins, my first solution was not event triggered functions like lambdas, but persistent sandboxes/containers that would subscribe to the same SSE endpoint as the client. it would more closely match the client plugin's stateful and persistent model. it would also allow both client and server plugins to also communicate to other external services and listen to events/webhooks/etc from other sources, with the user supplying credentials. macro could build oauth flows for the user to take, securely store the resulting credentials, and letting the sandboxed plugin use the credentials thorugh a trusted proxy.

but the only thing more expensive than hosting a bunch of small stateful servers is hosting a bunch of small unstrusted stateful servers. most sandbox providers are designed to be ephemeral, not 24x7. so for the majority use case of server plugins - automating work using macro primitives - it is overkill and the lambda style api is not only simpler for the developer its simpler for macro to operate.

but when it comes to integrating with granola, something needs to be watching for webhooks or polling its api, and storing new transcripts in macro. databases provides a very natural place to model integratinos, but the actual logic needs persistence. the simplest thing is to let the develoepr own that, and use macro sdk.

a middle ground might be to not integrate every single provider in the core, but build support for some common patterns like webhooks or polling. so granola could send a webhook to macro directly, but macro doesnt have any special understnading of granola, it just dumps the event into a database and custom plugins take it from there. or macro could be configured to poll a certain endpoint at some interval and store results in a database. so templates = plugins + databases + external events

theoretically this could allow every part of macro's product be broken into templates. including the unified inbox, channels, tasks, everything. because effectively everything is external events + database + automations + ui. so macro goes from a workspace product to a platform for building a workspace product of your own using templates, plus a set of beautifully curated and engineered templates that you dont need to buid yourself or can use as a starting point for your own.

macro, goes from being a product, becomes a stack on top of a platform for everyone to build on top of.

the complete/flagship product would not only be better than it is today, it would be completely modular and composable with third party pieces.

for developers wanting to build on top of macro, they should be provided the same level of capability and extensibility as we have when we are building the product on top of the platform.

the closest example i see of this today is opencode with opencode v2, they went under this exact transformation. they first built an advanced devtool with v1 with a decent develoepr plugin api, but the plugin api will always lag behind in capability of what the team has access to internally, so there will always be features that are impossible or way too complicated to add. but with opencode v2, and pi and some other harnesses trying to push for composability and extensibility by developers, are adopting this "everything is a plugin" approach to really open up the ecosystem. it's a massive transformation but ultimately it invites a completely different level of developer engagement

so heres the prototype i want.

forget the backend, lets just prototype this entire vision as static ui mockups added to the macro frontend

two client plugin ilnks added in the navigation, one a custom dashboard and another a form or wizard idk come up with ideas, hopefully something non trivial. then also show a databases link that open a page that shows all the databases, potentially namespaced/in a hierarchy. lets show 6 databases, opening any of them should open a data table (keep it pretty simple, we can imagine a proper spreadsheet/datagrid ui there), and dummy data. hardcode as much as possible. dont even bother with array or objects of data, just put everything in jsx as much as possible.

then show a plugins link in the settings that opens a small plugin dashboard in the settings. show server plugins, which events they listen to, what permissions they have, basic run stats, and run history with logs. client side plugins could show session history along with logs from that entire session. complex devtools - imagine their existence not their implementation. focus on the ui mockup.

also include an integrations link in the settings that shows configured webhooks, with examples of granola and some others, and links to a database for each integration where the webhook is dumped.
