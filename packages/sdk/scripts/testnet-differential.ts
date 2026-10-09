/**
 * Manual comparison of the deployed contract with the TypeScript model, on Stellar Testnet.
 *
 *   tsx scripts/testnet-differential.ts --wasm-dir <dir with access_policy.wasm and mock_token.wasm> \
 *       [--policies 30] [--seed 1] [--report report.json]
 *
 * It makes its own throwaway keys (in memory only, never printed or saved), funds them with friendbot, deploys the
 * contract and the test-token fixtures, creates random policies, and then for every policy and every subject asks the
 * contract (`evaluateOnChain`) and works the answer out with the model (`fetchSnapshot` + `evaluate`), both pinned to
 * the same ledger. Any disagreement is reported and makes the exit code 1.
 *
 * This talks to a public network and takes a few minutes, so it is not part of CI. It refuses to run against any
 * network that does not report the Testnet passphrase.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

import {
  Address,
  Asset,
  Keypair,
  nativeToScVal,
  Networks,
  Operation,
  scValToNative,
  type xdr,
} from '@stellar/stellar-sdk';

import {
  encodeConditions,
  evaluate,
  evaluateOnChain,
  fetchSnapshot,
  getPolicy,
  u64ToScVal,
  validateConditions,
  type CallContext,
  type Condition,
  type Decision,
  type PolicyRules,
} from '../src/index.js';
import { Rng } from './random-vectors.js';
import { assertTestnet, deployInstance, deployWasm, fund, invoke, log, server, sleep, submit } from './testnet-lib.js';


const arg = (name: string, fallback: string): string => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 && process.argv[i + 1] !== undefined ? (process.argv[i + 1] as string) : fallback;
};
const POLICIES = Number(arg('policies', '30'));
const SEED = Number(arg('seed', '1'));
const WASM_DIR = resolve(arg('wasm-dir', '../../target/wasm32v1-none/release'));
const REPORT = process.argv.includes('--report') ? resolve(arg('report', 'report.json')) : null;


// ---------------------------------------------------------------- the world

const MODE = { i128: 0, u32: 1, u64: 2, panics: 3 } as const;
const SUBJECT_NAMES = ['holder', 'nonholder', 'ghost1', 'ghost2'] as const;
type SubjectName = (typeof SUBJECT_NAMES)[number];

interface World {
  policyContract: string;
  tokens: Record<string, { address: string; kind: 'fungible' | 'collection' | 'odd'; balances: Partial<Record<SubjectName, bigint>> }>;
  subjects: Record<SubjectName, string>;
  ctx: CallContext;
  deployer: Keypair;
  wasm: { policy: string; mockToken: string };
}

const INTERESTING = [0n, 1n, 2n, 100n, 101n, 1_000_000n, 2n ** 127n - 1n];

async function buildWorld(rng: Rng): Promise<World> {
  const deployer = Keypair.random();
  const holder = Keypair.random();
  const nonholder = Keypair.random();
  const ghost1 = Keypair.random();
  const ghost2 = Keypair.random();
  log('funding three throwaway accounts with friendbot');
  await Promise.all([deployer, holder, nonholder].map((k) => fund(k.publicKey())));
  await sleep(6000);

  const read = (name: string) => readFileSync(resolve(WASM_DIR, name));
  log('deploying the access-policy contract');
  const policy = await deployWasm(deployer, read('access_policy.wasm'), []);
  log('uploading and deploying test tokens (fixtures)');
  const firstToken = await deployWasm(deployer, read('mock_token.wasm'), [nativeToScVal(MODE.i128, { type: 'u32' })]);
  const mockWasm = firstToken.wasmSha256;
  const mk = (mode: number) => deployInstance(deployer, mockWasm, [nativeToScVal(mode, { type: 'u32' })]);

  const tokens: World['tokens'] = {};
  tokens.coin0 = { address: firstToken.contractId, kind: 'fungible', balances: {} };
  tokens.coin1 = { address: await mk(MODE.i128), kind: 'fungible', balances: {} };
  tokens.coin2 = { address: await mk(MODE.i128), kind: 'fungible', balances: {} };
  tokens.set0 = { address: await mk(MODE.u32), kind: 'collection', balances: {} };
  tokens.set1 = { address: await mk(MODE.u32), kind: 'collection', balances: {} };
  tokens.wide = { address: await mk(MODE.u64), kind: 'odd', balances: {} };
  tokens.broken = { address: await mk(MODE.panics), kind: 'odd', balances: {} };
  tokens.nofn = { address: policy.contractId, kind: 'odd', balances: {} }; // a deployed contract with no `balance`

  log('deploying a Stellar asset contract and giving the holder a trustline and a balance');
  const asset = new Asset('SGT', deployer.publicKey());
  const assetContract = await submit(deployer, Operation.createStellarAssetContract({ asset }), true);
  const assetId = scValToNative(assetContract.returnValue as xdr.ScVal) as string;
  await submit(holder, Operation.changeTrust({ asset }), false);
  await submit(deployer, Operation.payment({ destination: holder.publicKey(), asset, amount: '1000' }), false);
  tokens.asset = { address: assetId, kind: 'fungible', balances: { holder: 1000n * 10_000_000n } };

  const subjects: Record<SubjectName, string> = {
    holder: holder.publicKey(),
    nonholder: nonholder.publicKey(), // exists, but has no trustline for the asset
    ghost1: ghost1.publicKey(), // does not exist on the network
    ghost2: ghost2.publicKey(),
  };

  log('setting balances on the test tokens');
  for (const [name, token] of Object.entries(tokens)) {
    if (name === 'asset' || token.kind === 'odd') continue;
    for (const who of SUBJECT_NAMES) {
      if (!rng.chance(0.75)) continue;
      const amount = token.kind === 'collection' ? BigInt(rng.pick([0, 1, 2, 3])) : rng.pick(INTERESTING);
      await invoke(deployer, token.address, 'set_balance', [new Address(subjects[who]).toScVal(), nativeToScVal(amount, { type: 'i128' })]);
      token.balances[who] = amount;
    }
  }

  const ctx: CallContext = { rpc: server, networkPassphrase: Networks.TESTNET, source: deployer.publicKey() };
  return { policyContract: policy.contractId, tokens, subjects, ctx, deployer, wasm: { policy: policy.wasmSha256, mockToken: mockWasm } };
}

// ---------------------------------------------------------------- random policies

function minimumNear(rng: Rng, balance: bigint | undefined, max: bigint): bigint {
  const candidates: bigint[] = [1n, 2n, 100n];
  if (balance !== undefined) candidates.push(balance, balance + 1n, balance - 1n, balance);
  const pick = rng.pick(candidates);
  return pick < 1n ? 1n : pick > max ? max : pick;
}

function randomConditions(rng: Rng, world: World, now: bigint, targetSubject: SubjectName | null): Condition[] {
  const names = Object.keys(world.tokens);
  const ofKind = (kind: string) => names.filter((n) => world.tokens[n]?.kind === kind);
  const subject = targetSubject ?? rng.pick(SUBJECT_NAMES);
  const count = targetSubject !== null ? 1 + rng.int(2) : 1 + rng.int(6);
  const list: Condition[] = [];
  for (let i = 0; i < count; i++) {
    const kind = rng.pick(['token', 'token', 'nft', 'window'] as const);
    if (kind === 'token' || kind === 'nft') {
      const isNft = kind === 'nft';
      let tokenName: string | undefined;
      let minVal: bigint = 1n;

      if (targetSubject !== null) {
        const validTokens = names.filter((n) => {
          const t = world.tokens[n];
          if (!t) return false;
          if (isNft && t.kind !== 'collection' && !rng.chance(0.15)) return false;
          if (!isNft && t.kind !== 'fungible' && !rng.chance(0.15)) return false;
          return (t.balances[targetSubject] ?? 0n) >= 1n;
        });
        if (validTokens.length > 0) {
          tokenName = rng.pick(validTokens);
          const balance = world.tokens[tokenName]!.balances[targetSubject]!;
          const max = isNft ? 4294967295n : 2n ** 127n - 1n;
          const candidates = [1n, 2n, 100n, balance, balance - 1n]
            .filter((x) => x >= 1n && x <= balance && x <= max);
          minVal = candidates.length === 0 ? 1n : rng.pick(candidates);
        }
      }

      if (!tokenName) {
        tokenName = rng.chance(0.85) ? rng.pick(ofKind(isNft ? 'collection' : 'fungible')) : rng.pick([...ofKind(isNft ? 'fungible' : 'collection'), ...ofKind('odd')]);
        const t = world.tokens[tokenName];
        if (!t) continue;
        minVal = minimumNear(rng, t.balances[subject], isNft ? 4294967295n : 2n ** 127n - 1n);
      }

      const t = world.tokens[tokenName];
      if (!t) continue;

      if (isNft) {
        list.push({ type: 'nft_balance', collection: t.address, min: Number(minVal) });
      } else {
        list.push({ type: 'token_balance', token: t.address, min: minVal });
      }
    } else {
      for (;;) {
        const edges = [now - 30n, now - 5n, now, now + 5n, now + 30n, now + 3600n].map((v) => (v < 0n ? 0n : v));
        const from = rng.chance(0.3) ? null : rng.pick(edges);
        const to = rng.chance(0.3) ? null : rng.pick(edges);
        if (from === null && to === null) continue;
        if (from !== null && to !== null && from >= to) continue;

        if (targetSubject !== null) {
          if (from !== null && from > now) continue;
          if (to !== null && to < now + 30n) continue;
        }

        list.push({ type: 'time_window', notBefore: from, notAfter: to });
        break;
      }
    }
  }
  return list;
}

// ---------------------------------------------------------------- comparison

interface Row {
  policyId: number;
  subject: SubjectName;
  conditions: number;
  onChain: Decision;
  model: Decision;
  ledger: number;
  agree: boolean;
}

const same = (a: Decision, b: Decision) => a.allowed === b.allowed && a.version === b.version && a.failedIndex === b.failedIndex && a.reason === b.reason;
const show = (v: unknown) => JSON.stringify(v, (_k, x) => (typeof x === 'bigint' ? x.toString() : x));

/** `null` when the contract and the model could not be read from the same ledger in the allowed attempts. */
async function compare(world: World, policyId: number, rules: PolicyRules, subjectName: SubjectName): Promise<Row | null> {
  const subject = world.subjects[subjectName];
  for (let attempt = 0; attempt < 8; attempt++) {
    // Asked at the same moment, so they usually land on the same ledger; when they do not, ask again.
    const [onChain, read] = await Promise.all([
      evaluateOnChain(world.ctx, { contractId: world.policyContract, policyId, subject }),
      fetchSnapshot(world.ctx, { conditions: rules.conditions, subject, attempts: 8 }).catch(() => null),
    ]);
    if (read === null || onChain.ledgerSequence !== read.ledgerSequence) continue;
    const model = evaluate(rules, read.snapshot);
    return { policyId, subject: subjectName, conditions: rules.conditions.length, onChain: onChain.decision, model, ledger: read.ledgerSequence, agree: same(onChain.decision, model) };
  }
  return null;
}

async function main(): Promise<void> {
  if (!Number.isInteger(POLICIES) || POLICIES < 1 || !Number.isInteger(SEED)) throw new Error('--policies and --seed must be integers');
  const network = await assertTestnet();
  log(`Testnet, protocol ${network.protocolVersion}; ${POLICIES} policies, seed ${SEED}`);

  const rng = new Rng(SEED);
  const world = await buildWorld(rng);
  const rows: Row[] = [];
  const mismatches: Row[] = [];
  const unpaired: string[] = [];
  const codecProblems: string[] = [];

  for (let n = 0; n < POLICIES; n++) {
    const now = BigInt((await server.getLatestLedger()).closeTime);
    const targetSubject = rng.chance(0.7) ? rng.pick(SUBJECT_NAMES) : null;
    const conditions = randomConditions(rng, world, now, targetSubject);
    const valid = validateConditions(conditions, () => true);
    if (!valid.ok) throw new Error(`generated an invalid policy: ${valid.error}`);

    const { returnValue: created } = await invoke(world.deployer, world.policyContract, 'create', [new Address(world.deployer.publicKey()).toScVal(), encodeConditions(conditions)]);
    const policyId = Number(scValToNative(created as xdr.ScVal) as bigint);
    const updates = rng.chance(0.1) ? 1 + rng.int(2) : 0;
    for (let u = 0; u < updates; u++) await invoke(world.deployer, world.policyContract, 'update', [u64ToScVal(policyId), encodeConditions(conditions)]);
    if (rng.chance(0.1)) await invoke(world.deployer, world.policyContract, 'set_active', [u64ToScVal(policyId), nativeToScVal(false)]);

    const { policy } = await getPolicy(world.ctx, { contractId: world.policyContract, policyId });
    if (show(policy.conditions) !== show(conditions)) codecProblems.push(`policy ${policyId}: stored conditions differ from what was sent`);
    const rules: PolicyRules = { version: policy.version, active: policy.active, conditions: policy.conditions };

    for (const subject of SUBJECT_NAMES) {
      const row = await compare(world, policyId, rules, subject);
      if (row === null) {
        unpaired.push(`policy ${policyId}, ${subject}`);
        continue;
      }
      rows.push(row);
      if (!row.agree) mismatches.push(row);
    }
    log(`policy ${policyId} (${conditions.length} conditions, version ${policy.version}${policy.active ? '' : ', inactive'}): ${rows.length} comparisons so far, ${mismatches.length} disagreements, ${unpaired.length} unpaired`);
  }

  const reasons: Record<string, number> = {};
  for (const r of rows) reasons[r.onChain.reason] = (reasons[r.onChain.reason] ?? 0) + 1;
  const allowedCount = rows.filter((r) => r.onChain.allowed).length;
  const allowedShare = `${((allowedCount / rows.length) * 100).toFixed(1)}%`;

  const summary = {
    label: 'Recorded',
    network: `Stellar Testnet (protocol ${network.protocolVersion})`,
    ranAt: new Date().toISOString(),
    seed: SEED,
    policies: POLICIES,
    comparisons: rows.length,
    allowedDecisions: allowedCount,
    allowedShare,
    disagreements: mismatches.length,
    unpaired,
    codecProblems,
    decisionsByReason: reasons,
    contracts: { accessPolicy: world.policyContract, accessPolicyWasmSha256: world.wasm.policy, mockTokenWasmSha256: world.wasm.mockToken, tokens: Object.fromEntries(Object.entries(world.tokens).map(([k, v]) => [k, v.address])) },
    mismatches: mismatches.map((m) => ({ ...m })),
  };
  console.log('\n' + show({ ...summary, mismatches: undefined, contracts: undefined }));
  if (REPORT !== null) writeFileSync(REPORT, JSON.stringify(summary, (_k, x) => (typeof x === 'bigint' ? x.toString() : x), 2) + '\n');
  if (mismatches.length > 0 || codecProblems.length > 0) {
    for (const m of mismatches.slice(0, 5)) console.error('DISAGREEMENT', show(m));
    process.exitCode = 1;
  } else {
    console.log(`\nAgreement: the contract and the model gave the same answer for all ${rows.length} comparisons.`);
  }
}

main().catch((error: unknown) => {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
});
