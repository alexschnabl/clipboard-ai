import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const conf = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const version = conf.version;
const nsisDir = join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
const setup = readdirSync(nsisDir).find((f) => f.endsWith(`_${version}_x64-setup.exe`));
if (!setup) {
	console.error(`No ${version} NSIS installer in ${nsisDir}. Run the release build first.`);
	process.exit(1);
}
let signature;
try {
	signature = readFileSync(join(nsisDir, `${setup}.sig`), 'utf8').trim();
} catch {
	console.error(`${setup}.sig is missing. Build with the updater config and TAURI_SIGNING_PRIVATE_KEY set.`);
	process.exit(1);
}
const notesFlag = process.argv.indexOf('--notes');
const notes = notesFlag > -1 ? process.argv[notesFlag + 1] : `Clipboard AI ${version}`;
const assetName = setup.replace(/ /g, '.');
const url = `https://github.com/alexschnabl/clipboard-ai/releases/download/v${version}/${assetName}`;
const manifest = {
	version,
	notes,
	pub_date: new Date().toISOString(),
	platforms: {
		'windows-x86_64': { signature, url }
	}
};
const out = join(nsisDir, 'latest.json');
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`Wrote ${out}`);
if (assetName !== setup) console.log(`GitHub stores "${setup}" as "${assetName}"; latest.json points there.`);
