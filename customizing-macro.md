Macro is the shared workspace that provides complete context to your team and your agents. So what if Macro could also be customized to your needs and the way you work? And what if customizing Macro was ridiculously easy to do?

Macro plugins provide the ability to customize Macro with your own workflows and interfaces.

Let me show you how Macro plugins work.

The code required to build a plugin is a definePluin export in a single typescript file. All the configuration required for a Macro plugin goes here.

You can create two types of plugins, client and server. Let's start with server plugins.

Server plugins are typescript functions that react to events inside your macro workspace. You define which event they react to, and a function that receives the event as an argument, as well as the macro sdk.

This function can use the sdk to query any data or perform any action. For example, every time we receive something in the inbox, it automatically creates a task for it.

We can see it in action here, when I send a message here, i automatically see a task here in the other tab.

You can deploy the server plugin to run inside your macro workspace in the cloud, or you can also host these yourself, you simply need to provide the Macro workspace credentials.

(In the demo make it self hosted locally separate from Macro so we are not implementing the sandboxing and deployment stuff inside Macro. Macro backend stays unchanged, except for any changes we might need for webhooks or SSE)

You can also restrict the capabilities of the plugin by defining what it is allowed to do, and this will not only prevent the code from calling the restricted apis at runtime, it's also going to narrow the type of the sdk object so typescript is only aware of the methods that are authorized.

Next up is the client plugin. There are two types of client plugins.
First is the page plugin. This plugin simple receives a label and a solidjs component. This component is rendered inside the Macro UI as an entire page, with its own link in the sidebar. Within this Solid component you can render whatever UI you want, it gets rendered in isolation from the rest of the Macro app. Instead it can use the Macro SDK to query data, perform operations, and subscribe to changes for realtime updates to the UI.

For your convenince there is a small solidjs component library that matches Macro's aesthetic, so you can very quickly build plugins that integrate seamlessly into macro, with complete flexibility to build the interface however you want.

Sidebar plugins are the second type of client plugins that show up in the sidebar of an entity's page like this. These plugins can define which entity they care about, and the solidjs component will receive typed data about the entity it's rendered in. This allows building small custom interfaces inside Macro's primitives.

With workflows that react to events and interfaces that update in realtime, you can easily build highly interactive mini applications that live next to and inside your entire workspace and operate completely on top of Macro's primitives. The definePlugin api allows you to author these plugins in a cohesive composable way.

How the demo works

In the demo, definePlugin simply returns a startPlugin function that receives a way to listen for macro events from its infrastructure, and calls the plugins. its just a typescript function that can be deployed in any typescritp environment the user wants. it currently uses webhooks but can also support sse. the developer is free to deploy this wherever they want, as long as they also assume complete operational responsibilities of the plugin.

Eventually Macro could support managed server plugin hosting. But when the developer is free to host and operate it themselves that can also easily integrate it with other systems much more easily. macro sdk doesnt make any runtime assumptions so you can talk to external services inside the server plugins.

Client plugins NEED to be deployed to Macro so maybe defineClientPlugin and defineServerPlugin need to be separate. lets fo createServerPlugin and createClientPlugin. will need to update this entire doc in retrospect.

but from now on defineplugin is gone and createClientPlugin and createServerPlugin are in.

But in the demo, there is no deployment, the macro source code simply imports the createClientPlugin and renders the component in an isolated thingy somhow. however its doing it today. the production version will need an endpoint to deploy the entire plugin built code, and more endpoints to control and monitor it. a complete plugin control plane is beyond the scope of this demo and this approach works well enough to prove the point.

The sdk instance for both client and server require some additional endpoints in the backend which are made in the local macro version but none of these look anything like what we will end up in production.

The copmonent library is a library of three components that have been made to look like some macro stuff, this should only be a serious effort in my opinion if Macro itself ALSO starts using this component library.

The SDK currently only provides a limited number of realtime events to listen to on the client, but the SDK should also be something more built in macro features should be using. Eventually the sync engine could also be exposed through the SDK so that client plugins can share Macro's instant UX, although solid 2.0s optimistic stores makes it much easier than before to simply layer instant mutations on top of server derived state, but it still has to be added manually on top of the existing query code.

You should also be able to run macro client plugins inside your own solidjs apps, createClientPlugin could simple return a solidjs componetn that you render in any solid application with an instance of a macro sdk instance.
We could also demo a simple solid app running on another port in localhost that simply renders the client plugin component full page.

They can be deployed inside Macro if you want them in the UI, or you can deploy them wherever you want.

(browser extension maybe? that would allow plugin integration without any control plane additions to the macro backend, which means the macro backend only has to accomodate a more powerful SDK, plugins operate completely in userspace, and the create*Plugin apis are simply typesafe convenience helpers to write the code for the plugins. Eventually Macro can also manage the hosting and provide a simple control place CLI for a fuller experience.)

Roadmap

Plugin API
The Plugin API is an extension of the Macro SDK that allows authoring plugins in a nice typesafe way. It should allow granular type narrowing capabilities, helpful errors, broader support (e.g. React/Web components), expressive event filters, more types of client plugins.

SDK
Same as components - new Macro features should be using the SDK, which means new features get adopted by the SDK first. The more powerful the SDK, the more powerful the plugin capability ceiling. This also means that more built in features can be replicated or cloned by custom plugins, but that's a good thing - that indicates people are choosing to stay on Macro and customize it to their needs rather than move to a different product.

Components
Make the UI components available for client plugins better, closer to the actual macro UI, and ideally use it directly for any future macro features, and slowly refactor Macro to depend more and more on this component library. Dogfooding and advancing the library at the same time.

Platform
Most users and agents will definitely need managed hosting and an easy to use control plane, both for server and client plugins. This is a more ambitious product and is a slippery slope towards building a hosting platform like Vercel, so this shouldn't be the first resort.

Registry
A place to share and collaborate on plugins. Once you see 3 independent community build macro plugin registry you'll know it's time to build a first party registry. And once there are courses about how to build Macro plugins you'll know it's time to think of monetization.

Sync Engine
Expose the Macro sync engine to Client plugins. For client plugins deployed within Macro should be easier. For plugins hosted as browser extensions, the client side of the sync engine would be CDN hosted for plugins to import and use.

## Addendum

Now that I read the macro sdk readme, im realizing that it already provides a decent api to listen to events from webhooks. i dont think we need a server side plugin api at all for now. we might not need server plugins as a concept. "plugin" simply means a solidjs component rendered inside macro's interface with access to the macro sdk. maybe we dont need platform at all if this can just be a browser extension.
