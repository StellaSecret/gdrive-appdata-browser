#!/usr/bin/env bash
#
# ── 4. Marqueurs cargo-mutants oubliés ─────────────────────
# cargo-mutants applique une mutation, lance les tests, puis doit la
# restaurer — que la mutation ait été détectée ou non. Si cette étape
# échoue ou est interrompue (run interrompu, copier-coller manuel en
# investiguant un mutant non détecté...), la ligne mutée — généralement
# cassée — peut rester commitée à la place du code original.
# Cette vérification bloque le commit avant que ça ne se reproduise sans
# être remarqué.
set -uo pipefail

RUST_STAGED=$(git diff --cached --name-only --diff-filter=ACM | grep '\.rs$' || true)
if [ -n "$RUST_STAGED" ]; then
  MUTANT_HITS=$(echo "$RUST_STAGED" | xargs grep -l "changed by cargo-mutants" 2>/dev/null || true)
  if [ -n "$MUTANT_HITS" ]; then
    echo "❌ Marqueur cargo-mutants trouvé dans le code stagé — commit bloqué :"
    echo "$MUTANT_HITS"
    echo "   Restaurez la logique d'origine avant de commiter."
    exit 1
  fi
fi
