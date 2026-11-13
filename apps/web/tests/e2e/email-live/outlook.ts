import { type APIRequestContext, expect, type Page } from '@playwright/test';

/** A seeded user of `crates/outlook_emulator/seed.example.yaml`, whose
 * Outlook mailbox receives the replies. */
export const CAROL = { name: 'Carol Example', email: 'carol@example.com' };

export type OutlookUser = { name: string; email: string };

/**
 * How long Outlook mail takes to reach Macro on a local stack. Graph change
 * notifications need an HTTPS webhook, which the stack does not have, so
 * the mailbox sync reads a folder's changes on its own schedule: two
 * minutes after its last round finished (`email_sync_streams.next_run_at`
 * in `crates/email/src/outbound/mailbox_pg.rs`). Allow one full interval
 * plus the round itself and the push to the open inbox.
 */
export const OUTLOOK_SYNC = 3 * 60_000;

const emulator = (): string => {
  const url = process.env.OUTLOOK_EMULATOR_URL;
  if (!url) throw new Error('OUTLOOK_EMULATOR_URL is required');
  return url.replace(/\/$/, '');
};

/** A new Microsoft user with an empty Outlook mailbox, so each spec signs
 * up afresh: a Microsoft sign-up is what offers to connect Outlook. */
export async function newOutlookUser(
  request: APIRequestContext
): Promise<OutlookUser> {
  const id = Date.now().toString(36);
  const user = { name: `Ada ${id}`, email: `ada-${id}@example.com` };
  const response = await request.post(`${emulator()}/_emulator/mailboxes`, {
    data: user,
  });
  expect(response.ok()).toBe(true);
  return user;
}

/** Picks `user` on the emulator's page that stands in for Microsoft's
 * account picker and consent screen. */
async function pickMicrosoftAccount(page: Page, user: OutlookUser) {
  await page
    .getByRole('button', { name: new RegExp(user.email) })
    .click({ timeout: 60_000 });
}

/**
 * Signs up with Microsoft, connects the same account's Outlook mailbox on
 * onboarding's work email step (the mailbox is a separate grant, as at
 * Microsoft), skips the rest of onboarding and opens Mail.
 */
export async function openOutlookMailAs(page: Page, user: OutlookUser) {
  await page.goto('welcome');
  await page.getByRole('button', { name: 'Continue with Microsoft' }).click();
  await pickMicrosoftAccount(page, user);
  await page
    .getByRole('button', { name: 'Get started' })
    .click({ timeout: 60_000 });
  const connect = page.getByRole('button', { name: 'Continue with Microsoft' });
  const next = page.getByRole('button', { name: 'Continue', exact: true });
  // The steps before the work email step each continue.
  await expect(async () => {
    if (await next.isVisible()) await next.click();
    await expect(connect).toBeVisible({ timeout: 1_000 });
  }).toPass({ timeout: 60_000 });
  await connect.click();
  await pickMicrosoftAccount(page, user);
  await expect(
    page.getByRole('status').filter({ hasText: 'Work email connected' })
  ).toBeVisible({ timeout: 60_000 });
  await page.getByRole('button', { name: 'Bypass' }).click();
  await expect(page).not.toHaveURL(/\/(welcome|onboarding)/, {
    timeout: 30_000,
  });
  await page.goto('component/mail');
  await expect(page.getByRole('grid', { name: 'Email' })).toBeVisible({
    timeout: 60_000,
  });
}

/** Mail arrives in a user's Outlook mailbox. */
export async function deliver(
  request: APIRequestContext,
  to: { email: string },
  mail: { from: string; subject: string; text: string }
) {
  const response = await request.post(
    `${emulator()}/_emulator/mailboxes/${to.email}/messages`,
    { data: mail }
  );
  expect(response.ok()).toBe(true);
}

type Message = { subject: string; folder: string; is_draft: boolean };

async function messagesIn(
  request: APIRequestContext,
  user: { email: string }
): Promise<Message[]> {
  const response = await request.get(
    `${emulator()}/_emulator/mailboxes/${user.email}/messages`
  );
  expect(response.ok()).toBe(true);
  return response.json();
}

/** The subjects in a user's Outlook inbox. */
export async function inboxOf(
  request: APIRequestContext,
  user: { email: string }
): Promise<string[]> {
  return (await messagesIn(request, user))
    .filter((message) => message.folder === 'Inbox')
    .map((message) => message.subject);
}

/** The subjects of the drafts in a user's Outlook mailbox. */
export async function draftsIn(
  request: APIRequestContext,
  user: { email: string }
): Promise<string[]> {
  return (await messagesIn(request, user))
    .filter((message) => message.is_draft)
    .map((message) => message.subject);
}
