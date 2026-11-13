import { expect, test } from '@playwright/test';
import { uniqueSubject } from './gmail';
import {
  CAROL,
  deliver,
  draftsIn,
  inboxOf,
  newOutlookUser,
  OUTLOOK_SYNC,
  openOutlookMailAs,
} from './outlook';

// Mail reaches Macro on the Outlook sync's schedule, not at once, and the
// last spec waits for it twice.
test.describe.configure({ timeout: 3 * OUTLOOK_SYNC });

test('new Outlook mail appears in the inbox without a reload', async ({
  page,
  request,
}) => {
  const ada = await newOutlookUser(request);
  await openOutlookMailAs(page, ada);
  const subject = uniqueSubject('Live delivery');

  await deliver(request, ada, {
    from: 'Dana <dana@example.com>',
    subject,
    text: 'Arrived while the inbox was open.',
  });

  await expect(
    page.getByRole('row', { name: new RegExp(subject) })
  ).toBeVisible({ timeout: OUTLOOK_SYNC });
});

test('an Outlook reply reaches its recipient and leaves no draft behind', async ({
  page,
  request,
}) => {
  const ada = await newOutlookUser(request);
  await openOutlookMailAs(page, ada);
  const subject = uniqueSubject('Lunch');
  await deliver(request, ada, {
    from: `Carol <${CAROL.email}>`,
    subject,
    text: 'Noon on Friday?',
  });

  await page
    .getByRole('row', { name: new RegExp(subject) })
    .click({ timeout: OUTLOOK_SYNC });
  await page.getByRole('button', { name: /Reply\.\.\./ }).click();
  await page.getByRole('textbox', { name: /^Reply/ }).fill('Friday works.');
  await page.getByRole('button', { name: 'Send email' }).click();

  await expect
    .poll(
      async () =>
        (await inboxOf(request, CAROL)).some((received) =>
          received.endsWith(subject)
        ),
      { timeout: 60_000 }
    )
    .toBe(true);
  expect(await draftsIn(request, ada)).toEqual([]);
  await page.goto('component/mail');
  await page
    .getByRole('navigation', { name: 'Email tabs' })
    .getByRole('button', { name: 'Drafts' })
    .click();
  await expect(page.getByText('No drafts')).toBeVisible({ timeout: 30_000 });
});

test('an Outlook email marked done stays hidden when new mail arrives', async ({
  page,
  request,
}) => {
  const ada = await newOutlookUser(request);
  await openOutlookMailAs(page, ada);
  const done = uniqueSubject('Receipt');
  await deliver(request, ada, {
    from: 'Shop <shop@example.com>',
    subject: done,
    text: 'Thanks for your order.',
  });
  const doneRow = page.getByRole('row', { name: new RegExp(done) });
  await doneRow.click({ timeout: OUTLOOK_SYNC });
  await page.getByRole('button', { name: 'Mark done' }).click();
  await expect(doneRow).toHaveCount(0);
  await page.goto('component/mail');

  const later = uniqueSubject('Later');
  await deliver(request, ada, {
    from: 'Dana <dana@example.com>',
    subject: later,
    text: 'Another one.',
  });

  await expect(page.getByRole('row', { name: new RegExp(later) })).toBeVisible({
    timeout: OUTLOOK_SYNC,
  });
  await expect(page.getByRole('row', { name: new RegExp(done) })).toHaveCount(
    0
  );
});
