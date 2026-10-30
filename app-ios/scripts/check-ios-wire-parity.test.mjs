// node --test app-ios/scripts/check-ios-wire-parity.test.mjs
//
// The ruler's own tests: each column must bite on a hand-built machine, and
// stay quiet on one that answers everything.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
	typeBody,
	unionVariants,
	structFields,
	handledCases,
	producedResults,
	onlyNull,
	measure,
	strictFailures,
	render
} from './check-ios-wire-parity.mjs';

const OPS = `// generated
import type { X } from "./X";
/** doc */
export type DemoOperation = { "type": "read_store" } | { "type": "write_list", items: Array<string>, } | { "type": "new_op", id: number, };`;

const RESULTS = `export type DemoShellResult = { "type": "loaded",
/** a comment with a | pipe and { brace */
items: Array<string>, flag: boolean, maybe?: number, note: string | null, risk: DemoRisk, } | { "type": "written" } | { "type": "failed_write" };`;

const RISK = `export type DemoRisk = { is_contract: boolean | null, own: boolean, known_name: string | null, };`;

const SWIFT_PARTIAL = `
final class DemoExecutor {
    private static let written = CoreJSON.string(["type": "written"])
    func perform(_ op: [String: Any]) async -> String {
        switch op["type"] as? String ?? "" {
        case "read_store":
            // "type": "failed_write" in a comment does not count
            return CoreJSON.string([
                "type": "loaded",
                "items": ["a", "b"],
                "note": NSNull(),
                "risk": ["is_contract": NSNull()],
            ])
        case "write_list", "other":
            return Self.written
        default:
            return Self.written
        }
    }
}`;

const SWIFT_FULL = `
final class DemoExecutor {
    private static let newOp = "new_op"
    func perform(_ op: [String: Any]) async -> String {
        switch op["type"] as? String ?? "" {
        case "read_store":
            var out: [String: Any] = ["type": "loaded", "items": [], "note": NSNull()]
            out["flag"] = true
            out["maybe"] = 1
            out["risk"] = ["is_contract": true, "own": false, "known_name": NSNull()]
            return CoreJSON.string(out)
        case "write_list":
            return CoreJSON.string(["type": "written"])
        case Self.newOp:
            return CoreJSON.string(["type": "failed_write"])
        default:
            return ""
        }
    }
}`;

function io(swift) {
	const gen = { 'DemoOperation.ts': OPS, 'DemoShellResult.ts': RESULTS, 'DemoRisk.ts': RISK };
	return {
		listGen: () => Object.keys(gen),
		readGen: (n) => gen[n] ?? null,
		readSwift: (rel) => (rel === 'Demo.swift' ? swift : null)
	};
}

test('typeBody skips imports and doc comments', () => {
	assert.match(typeBody(OPS), /^\{ "type": "read_store" \}/);
});

test('unionVariants reads tags and fields, optional = ? or | null', () => {
	const v = unionVariants(typeBody(RESULTS));
	assert.deepEqual(v.map((x) => x.tag), ['loaded', 'written', 'failed_write']);
	const loaded = Object.fromEntries(v[0].fields.map((f) => [f.name, f.optional]));
	assert.deepEqual(loaded, { items: false, flag: false, maybe: true, note: true, risk: false });
});

test('structFields reads a plain struct and refuses a union', () => {
	assert.deepEqual(structFields(RISK).map((f) => f.name), ['is_contract', 'own', 'known_name']);
	assert.equal(structFields(RESULTS), null);
});

test('handledCases reads multi-arm cases and constant cases', () => {
	assert.deepEqual([...handledCases(SWIFT_PARTIAL)].sort(), ['other', 'read_store', 'write_list']);
	assert.ok(handledCases(SWIFT_FULL).has('new_op'));
});

test('producedResults reads literal keys and later assignments', () => {
	const p = producedResults(SWIFT_FULL);
	assert.deepEqual([...p.get('loaded')].sort(), ['flag', 'items', 'maybe', 'note', 'risk']);
});

test('onlyNull: never written, or only NSNull()', () => {
	assert.equal(onlyNull('"a": NSNull(),', 'a'), true);
	assert.equal(onlyNull('"a": x,', 'a'), false);
	assert.equal(onlyNull('', 'a'), true);
});

test('measure: a partial executor is caught in every column', () => {
	const strip = (s) => s.replace(/\/\/.*$/gm, '');
	const [row] = measure(io(strip(SWIFT_PARTIAL)), { Demo: ['Demo.swift'] });
	assert.deepEqual(row.unanswered, ['new_op']);
	assert.deepEqual(row.neverProduced, ['failed_write']);
	assert.deepEqual(row.required, ['loaded.flag']);
	assert.deepEqual(row.optional, ['loaded.maybe']);
	assert.deepEqual(row.payload, ['loaded.risk.own', 'loaded.risk.known_name (opt)']);
	const fails = strictFailures([row]);
	assert.equal(fails.length, 3);
	assert.match(render([row]), /unanswered ops: 1/);
});

test('measure: comments never count as a produced result', () => {
	const [row] = measure(io(SWIFT_PARTIAL), { Demo: ['Demo.swift'] });
	assert.ok(row.neverProduced.includes('failed_write'));
});

test('measure: a complete executor passes --strict', () => {
	const [row] = measure(io(SWIFT_FULL), { Demo: ['Demo.swift'] });
	assert.deepEqual(row.unanswered, []);
	assert.deepEqual(row.required, []);
	assert.deepEqual(row.payload, []);
	assert.deepEqual(strictFailures([row]), []);
});

test('measure: an unmapped machine and a missing file fail --strict', () => {
	const unmapped = measure(io(SWIFT_FULL), {});
	assert.equal(unmapped[0].status, 'unmapped');
	const missing = measure(io(SWIFT_FULL), { Demo: ['Gone.swift'] });
	assert.equal(missing[0].status, 'missing-file');
	assert.equal(strictFailures(unmapped).length, 1);
	assert.ok(strictFailures(missing).length >= 1);
});

test('measure: a not-iOS machine is reported, never failed', () => {
	const rows = measure(io(SWIFT_FULL), { Demo: null });
	assert.equal(rows[0].status, 'not-ios');
	assert.deepEqual(strictFailures(rows), []);
});
