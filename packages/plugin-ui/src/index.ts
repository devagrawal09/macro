/**
 * @macro/plugin-ui — standalone SolidJS 2 UI kit for Macro client plugins.
 *
 * Exactly 16 top-level component exports. Types are exported alongside for
 * authoring ergonomics but are erased at runtime.
 */

// 1
export { PluginUIProvider, type PluginUIProviderProps } from "./provider";
// 2
export { Text, type TextProps, type TextRole, type TextTone, type TextAs } from "./text";
// 3
export { Stack, type StackProps } from "./stack";
// 4
export { Button, type ButtonProps, type ButtonVariant, type ControlSize } from "./button";
// 5
export { IconButton, type IconButtonProps } from "./icon-button";
// 6
export { FormField, type FormFieldProps } from "./form-field";
// 7
export { TextField, type TextFieldProps } from "./text-field";
// 8
export { TextArea, type TextAreaProps } from "./text-area";
// 9
export { Surface, type SurfaceProps } from "./surface";
// 10
export { Badge, type BadgeProps } from "./badge";
// 11 (List.Item is a compound member, not a separate top-level export)
export { List, type ListProps, type ListItemProps } from "./list";
// 12
export { Spinner, type SpinnerProps } from "./spinner";
// 13
export { EmptyState, type EmptyStateProps } from "./empty-state";
// 14
export { ErrorState, type ErrorStateProps } from "./error-state";
// 15
export { HostAction, type HostActionProps } from "./host-action";
// 16
export { HostLink, type HostLinkProps } from "./host-link";

// Shared contracts (types only).
export type { PluginIcon, PluginIconProps } from "./icon";
export type {
	ClientHostIntent,
	ClientEntityRef,
	DispatchIntent,
} from "./intent";
export type {
	PluginThemeContractV1,
	PluginThemeTokensV1,
	PluginSurfaceContextV1,
	PluginPlacement,
	PluginContainerSize,
	PluginDensity,
	PluginContrast,
	PluginMotion,
} from "./theme";
export { MACRO_LIGHT_TOKENS, MACRO_DARK_TOKENS, TOKEN_CSS_PROPERTIES, resolveTheme } from "./theme";
