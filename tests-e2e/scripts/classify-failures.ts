#!/usr/bin/env bun
/**
 * Classification automatique des échecs Playwright pour dispatch aux agents
 * Claude spécialisés. Lit un rapport JSON Playwright et produit un fichier
 * JSON enrichi avec:
 *   - errorType (catégorie machine)
 *   - suggestedAgent (agent cible)
 *   - suggestedFix (hint pour le fix)
 *   - relatedFiles (chemins à inspecter)
 */
import fs from 'node:fs';

type ErrorType =
  | 'selector-missing'
  | 'navigation-timeout'
  | 'backend-5xx'
  | 'backend-4xx'
  | 'otp-not-received'
  | 'feature-missing'
  | 'unexpected-dom'
  | 'kratos-flow-error'
  | 'kaya-protocol-error'
  | 'unknown';

interface ClassifiedFailure {
  testId: string;
  specFile: string;
  testName: string;
  errorType: ErrorType;
  rootCauseHint: string;
  suggestedAgent:
    | 'kaya-rust-implementer'
    | 'general-purpose-frontend'
    | 'general-purpose-backend'
    | 'devops-engineer'
    | 'manual-review';
  suggestedFix: string;
  relatedFiles: string[];
  screenshot?: string;
  trace?: string;
}

const resultsFile = process.argv[2] ?? 'reports/results.json';
const outputFile = process.argv[3] ?? 'reports/failures.classified.json';

if (process.argv.includes('--help') || process.argv.includes('-h')) {
  process.stdout.write(
    'Usage: bun run scripts/classify-failures.ts [resultsFile] [outputFile]\n' +
      '  resultsFile  chemin vers results.json Playwright (défaut: reports/results.json)\n' +
      '  outputFile   chemin de sortie JSON classifié (défaut: reports/failures.classified.json)\n',
  );
  process.exit(0);
}

if (!fs.existsSync(resultsFile)) {
  process.stderr.write(`[classify] ${resultsFile} introuvable\n`);
  process.exit(1);
}

const raw = JSON.parse(fs.readFileSync(resultsFile, 'utf8'));
const out: ClassifiedFailure[] = [];

for (const suite of raw.suites ?? []) {
  walkSuite(suite, '');
}

fs.writeFileSync(outputFile, JSON.stringify(out, null, 2));
process.stdout.write(`${out.length} failures classified → ${outputFile}\n`);

function walkSuite(suite: any, parentPath: string): void {
  for (const spec of suite.specs ?? []) {
    for (const test of spec.tests ?? []) {
      for (const result of test.results ?? []) {
        if (result.status === 'failed' || result.status === 'timedOut') {
          out.push(classify(spec, test, result));
        }
      }
    }
  }
  for (const s of suite.suites ?? []) {
    walkSuite(s, `${parentPath}/${s.title}`);
  }
}

function classify(spec: any, test: any, result: any): ClassifiedFailure {
  const err = (result.error?.message ?? '') + ' ' + (result.error?.stack ?? '');
  let errorType: ErrorType = 'unknown';
  let agent: ClassifiedFailure['suggestedAgent'] = 'manual-review';
  let hint = 'unknown failure';
  let fix = 'Inspecter trace manuellement';
  const related: string[] = [spec.file];

  if (/TimeoutError.*locator|waitForSelector|element.*not.*found/i.test(err)) {
    errorType = 'selector-missing';
    agent = 'general-purpose-frontend';
    hint = 'Sélecteur Playwright attendu introuvable dans le DOM Angular';
    fix = 'Inspecter le composant Angular correspondant, mettre à jour le page-object';
    related.push('INFRA/frontend/src/app/**');
  } else if (/waitForOtp.*timeout|OTP introuvable/i.test(err)) {
    errorType = 'otp-not-received';
    agent = 'general-purpose-backend';
    hint = 'Aucun email reçu côté Mailpit — notifier-ms ou Kratos courriel cassé';
    fix = 'Vérifier Kratos courrier_disable=false + notifier-ms SMTP → mailpit:1025';
    related.push('INFRA/ory/kratos/config/kratos.yml', '/tmp/notifier-ms.log');
  } else if (/protocol parse error|expected array frame|unknown command/i.test(err)) {
    errorType = 'kaya-protocol-error';
    agent = 'kaya-rust-implementer';
    hint = 'Commande ou frame RESP3 non supportée par KAYA';
    fix = 'Étendre le dispatcher RESP3 dans kaya-protocol/commands';
    related.push('INFRA/kaya/src/**');
  } else if (/5\d\d|InternalServerError|HTTP 5/i.test(err)) {
    errorType = 'backend-5xx';
    agent = 'general-purpose-backend';
    hint = 'Backend Java a retourné 5xx — voir /tmp/*.log';
    fix = 'Lire /tmp/auth-ms.log ou /tmp/poulets-api.log pour la stacktrace';
    related.push('/tmp/auth-ms.log', '/tmp/poulets-api.log');
  } else if (/expect.*toHaveURL.*timeout/i.test(err) && /auth|login|dashboard/.test(err)) {
    errorType = 'kratos-flow-error';
    agent = 'general-purpose-backend';
    hint = 'Redirection Kratos boucle ou stagne';
    fix = 'Vérifier kratos.yml selfservice.flows + identity.schemas';
    related.push('INFRA/ory/kratos/config/kratos.yml');
  } else if (/4\d\d|BadRequest|HTTP 4/i.test(err)) {
    errorType = 'backend-4xx';
    agent = 'general-purpose-backend';
    hint = 'Backend a retourné 4xx — probable payload invalide ou auth manquante';
    fix = 'Inspecter trace HAR puis comparer au contrat OpenAPI';
    related.push('INFRA/docs/openapi/**');
  } else if (/navigation.*timeout|net::ERR_/i.test(err)) {
    errorType = 'navigation-timeout';
    agent = 'devops-engineer';
    hint = 'Le frontend/BFF ne répond pas dans les temps (stack DOWN ?)';
    fix = 'Lancer /status-faso et /stack-up si besoin';
    related.push('INFRA/docker/compose/podman-compose.yml');
  }

  const title = test.title ?? spec.title ?? '(unknown)';
  return {
    testId: `${spec.file}::${title}`,
    specFile: spec.file,
    testName: title,
    errorType,
    rootCauseHint: hint,
    suggestedAgent: agent,
    suggestedFix: fix,
    relatedFiles: related,
    screenshot: result.attachments?.find((a: any) => a.name === 'screenshot')?.path,
    trace: result.attachments?.find((a: any) => a.name === 'trace')?.path,
  };
}
