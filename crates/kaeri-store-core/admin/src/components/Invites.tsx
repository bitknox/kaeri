import { useEffect, useState } from 'react';
import type { Act } from '../App';
import { api } from '../api';
import { day, on } from '../format';
import {
	type InviteCreated,
	type InviteLasts,
	type InviteUses,
	type Member,
	NO_END_DATE,
	type PendingInvite,
} from '../types';
import { Confirm, Row } from './Row';
import { Section } from './Section';
import type { Status } from './StatusLine';

const USES: [InviteUses, string][] = [
	['once', 'Once'],
	['five', '5 times'],
	['any', 'Any number of times'],
];

const LASTS: [InviteLasts, string][] = [
	['day', '1 day'],
	['week', '7 days'],
	['month', '30 days'],
	['forever', 'No end date'],
];

const DAY = 86_400;
const SECONDS: Record<InviteLasts, number> = {
	day: DAY,
	week: 7 * DAY,
	month: 30 * DAY,
	forever: 0,
};

/** How many times a choice works, as the store counts it: 0 for any number. */
const MAX: Record<InviteUses, number> = { once: 1, five: 5, any: 0 };

/** What an invite allows, read back in a sentence: under the form, and in the made box. */
export function works(maxUses: number, expiresAt: number, withYear = false): string {
	const forever = expiresAt >= NO_END_DATE;
	const until = day(expiresAt, withYear);
	if (maxUses === 1) {
		return forever
			? 'Works once, for the person you send it to, until you cancel it.'
			: `Works once, for the person you send it to. Until ${until}.`;
	}
	if (maxUses === 0) {
		return forever
			? 'Anyone who has this link can join until you cancel it.'
			: `Anyone who has this link can join, until ${until}.`;
	}
	return forever
		? `Works ${maxUses} times, until you cancel it.`
		: `Works ${maxUses} times, until ${until}.`;
}

/** How many times an invite was used, against how many it allows. */
export function used(uses: number, maxUses: number): string {
	if (maxUses > 0 && uses > 0) return `Used ${uses} of ${maxUses}`;
	if (uses === 0) return 'Not used yet';
	return uses === 1 ? 'Used once' : `Used ${uses} times`;
}

const ENDED_AS = { used_up: 'Used up', ran_out: 'Ended', cancelled: 'Cancelled' } as const;

/** The line of facts under an invite's name. */
function facts(invite: PendingInvite): string {
	const end = invite.ended;
	const when = end
		? `${ENDED_AS[end.reason]} ${on(end.at)}`
		: invite.expires_at >= NO_END_DATE
			? 'Until you cancel it'
			: `Until ${day(invite.expires_at)}`;
	return `${used(invite.uses ?? 0, invite.max_uses ?? 1)} · ${when}`;
}

const label = (invite: PendingInvite) =>
	invite.kind === 'device' ? 'Device' : invite.role === 'owner' ? 'Owner' : 'Member';

export function Invites({
	invites,
	ended,
	contact,
	me,
	act,
	say,
}: {
	invites: PendingInvite[];
	ended: PendingInvite[];
	/** The store's contact, which a link that works any number of times needs. */
	contact?: string;
	me?: Member;
	act: Act;
	say: (text: string, tone?: Status['tone']) => void;
}) {
	const [name, setName] = useState('');
	const [owner, setOwner] = useState(false);
	const [uses, setUses] = useState<InviteUses>('once');
	const [lasts, setLasts] = useState<InviteLasts>('week');
	const [made, setMade] = useState<(InviteCreated & { name: string }) | null>(null);
	const [busy, setBusy] = useState(false);
	const [copied, setCopied] = useState(false);

	// An owner invite acts as an owner, so it works once.
	const chosen: InviteUses = owner ? 'once' : uses;
	const blocked = chosen === 'any' && !contact;

	useEffect(() => {
		if (!copied) return;
		const timer = window.setTimeout(() => setCopied(false), 1500);
		return () => window.clearTimeout(timer);
	}, [copied]);

	const make = async (
		who: string,
		role: 'owner' | 'member',
		times: InviteUses,
		length: InviteLasts,
	) => {
		setBusy(true);
		let created: InviteCreated | null = null;
		const ok = await act(
			async () => {
				created = await api.invite(who, role, times, length);
			},
			`Made an invite link for ${who}.`,
			() => void make(who, role, times, length),
		);
		setBusy(false);
		if (ok && created) {
			setMade({ ...(created as InviteCreated), name: who });
			setCopied(false);
			setName('');
			setOwner(false);
			setUses('once');
			setLasts('week');
		}
	};

	const create = (event: React.FormEvent) => {
		event.preventDefault();
		const who = name.trim();
		if (who && !busy && !blocked) void make(who, owner ? 'owner' : 'member', chosen, lasts);
	};

	// The link just made stays while its invite works. Once it is used up, ends or is cancelled it
	// works no more, so it goes.
	const shown = made && invites.some((invite) => invite.id === made.id) ? made : null;

	const copy = async () => {
		if (!shown) return;
		try {
			await navigator.clipboard.writeText(shown.link);
			setCopied(true);
			say(`Copied the link for ${shown.name}.`);
		} catch {
			say('The browser would not copy it. Select the link and copy it by hand.', 'alert');
		}
	};

	const expires =
		lasts === 'forever' ? NO_END_DATE : Math.floor(Date.now() / 1000) + SECONDS[lasts];
	const line = blocked ? (
		<p className="invite-line appear alert-text" key="contact">
			Add a contact for this store first. A link that works any number of times can be passed on,
			like the store's own link. <a href="#joining">Add a contact</a>
		</p>
	) : (
		<p className="invite-line dim">
			{owner && 'Owner invites work once. '}
			{works(MAX[chosen], expires)}
		</p>
	);

	return (
		<Section
			id="invites"
			title="Invite someone"
			lede="An invite link adds people to this store. Each person gets their own token for each device. Opening a link in a browser shows how to join without using it."
		>
			<form className="invite-form" onSubmit={create}>
				<label className="field invite-name">
					<span>
						{chosen === 'once'
							? 'Their name, as the members list will show it'
							: 'A name for this invite, such as a group. Each person adds their own name when they join.'}
					</span>
					<input
						value={name}
						maxLength={64}
						autoComplete="off"
						spellCheck={false}
						onChange={(event) => setName(event.target.value)}
					/>
				</label>
				<Choice
					legend="Can be used"
					name="invite-uses"
					options={USES}
					value={chosen}
					onChange={setUses}
					locked={owner}
				/>
				<Choice
					legend="Works for"
					name="invite-lasts"
					options={LASTS}
					value={lasts}
					onChange={setLasts}
				/>
				<label className="check">
					<input
						type="checkbox"
						checked={owner}
						onChange={(event) => setOwner(event.target.checked)}
					/>
					<span>As an owner, who can invite, remove and publish</span>
				</label>
				{line}
				<button
					type="submit"
					className="btn invite-submit"
					disabled={busy || blocked || !name.trim()}
				>
					{busy ? 'Making the link…' : 'Make an invite link'}
				</button>
			</form>

			{shown && (
				<fieldset className="made appear" aria-label={`Invite link for ${shown.name}`}>
					<p className="caps made-for">For {shown.name}</p>
					<p className="made-link">{shown.link}</p>
					<p className="made-works">{works(shown.max_uses ?? 1, shown.expires_at, true)}</p>
					<div className="actions">
						<button type="button" className="btn btn-small swaps" onClick={copy}>
							<span className={copied ? 'swap swap-out' : 'swap'} aria-hidden={copied}>
								Copy link
							</span>
							<span className={copied ? 'swap' : 'swap swap-out'} aria-hidden={!copied}>
								Copied
							</span>
						</button>
						<button type="button" className="btn btn-small btn-quiet" onClick={() => setMade(null)}>
							Done
						</button>
					</div>
				</fieldset>
			)}

			<div className="pending">
				<h3 className="pending-h">Invite links</h3>
				{invites.length === 0 && (
					<p className="pending-none dim">No invite links. A link shows here until it ends.</p>
				)}
				{invites.map((invite) => (
					<InviteLine key={invite.id} invite={invite} me={me} act={act} />
				))}
			</div>

			{ended.length > 0 && (
				<div className="pending pending-ended">
					<h3 className="pending-h">Ended in the last 30 days</h3>
					{ended.map((invite) => (
						<InviteLine key={invite.id} invite={invite} me={me} act={act} />
					))}
				</div>
			)}
		</Section>
	);
}

/** A segmented choice, drawn from radio buttons so it reads and moves as one control. */
function Choice<T extends string>({
	legend,
	name,
	options,
	value,
	onChange,
	locked = false,
}: {
	legend: string;
	name: string;
	options: [T, string][];
	value: T;
	onChange: (value: T) => void;
	/** Only the chosen option can be picked, and the rest dim. */
	locked?: boolean;
}) {
	return (
		<fieldset className="choice">
			<legend className="choice-legend">{legend}</legend>
			<div className={locked ? 'seg seg-locked' : 'seg'}>
				{options.map(([option, text]) => (
					<label key={option}>
						<input
							type="radio"
							name={name}
							value={option}
							checked={value === option}
							disabled={locked && value !== option}
							onChange={() => onChange(option)}
						/>
						<span>{text}</span>
					</label>
				))}
			</div>
		</fieldset>
	);
}

function InviteLine({ invite, me, act }: { invite: PendingInvite; me?: Member; act: Act }) {
	const [open, setOpen] = useState(false);
	const joined = invite.joined ?? [];
	const stayed = joined.filter((use) => !use.removed).length;
	const list = `invite-uses-${invite.id}`;

	const actions = (
		<>
			{joined.length > 0 && (
				<button
					type="button"
					className="btn btn-small btn-quiet"
					aria-expanded={open}
					aria-controls={list}
					onClick={() => setOpen(!open)}
				>
					Uses
				</button>
			)}
			{!invite.ended && (
				<Confirm
					label="Cancel"
					confirm={`Cancel the invite for ${invite.name}`}
					onConfirm={() =>
						act(
							() => api.cancelInvite(invite.id),
							stayed === 0
								? `Cancelled the invite for ${invite.name}. The link no longer works.`
								: stayed === 1
									? 'Invite cancelled. The person who joined stays a member.'
									: `Invite cancelled. The ${stayed} people who joined stay members.`,
						)
					}
				/>
			)}
		</>
	);

	return (
		<div className={invite.ended ? 'invite-row is-ended appear' : 'invite-row'}>
			<Row label={label(invite)} actions={actions}>
				<h3>{invite.kind === 'device' ? `${invite.name}, another device` : invite.name}</h3>
				<p className="facts">{facts(invite)}</p>
				{open && joined.length > 0 && (
					<ul
						id={list}
						className="devices uses appear"
						aria-label={`Who joined with the invite for ${invite.name}`}
					>
						{joined.map((use) => (
							<li key={`${use.member}-${use.used_at}`}>
								<span className="device-label">
									{use.name} · joined {on(use.used_at)}
								</span>
								{use.removed ? (
									<span className="dim">Removed</span>
								) : (
									use.member !== me?.id &&
									invite.kind === 'member' && (
										<Confirm
											label="Remove"
											confirm={`Remove ${use.name}`}
											onConfirm={() =>
												act(
													() => api.removeMember(use.member),
													`Removed ${use.name}. Their devices no longer work with this store.`,
												)
											}
										/>
									)
								)}
							</li>
						))}
					</ul>
				)}
			</Row>
		</div>
	);
}
