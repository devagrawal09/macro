import type {
  InstallListing,
  TemplateVariable,
} from '@service-agent-harness/generated/schemas';

/**
 * A routine install the server can't fill in yet: a new major added
 * variables its person hasn't decided on.
 */
export class MissingRoutineFills extends Error {
  constructor(readonly missing: readonly TemplateVariable[]) {
    super('This routine needs a few choices first.');
    this.name = 'MissingRoutineFills';
  }
}

/** Keep a referenced item, or leave it out as plain text. */
export type ReferenceDecision = 'keep' | 'leave_out';

/**
 * What an install sends for the person's decisions: each decided reference
 * as a fill (`shared` keeps it, `choice` leaves it out), and their time zone.
 */
export function installFills(args: {
  decisions: ReadonlyMap<string, ReferenceDecision>;
  /** Left out to keep the one stored at install. */
  timezone: string | undefined;
}): InstallListing {
  return {
    ...(args.timezone && { timezone: args.timezone }),
    // A copy starts switched off, until its person has looked at it.
    enabled: false,
    variables: Object.fromEntries(
      [...args.decisions].map(([key, decision]) => [
        key,
        decision === 'keep' ? 'shared' : 'choice',
      ])
    ),
  };
}
