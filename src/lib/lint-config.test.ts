import { describe, expect, it } from "vitest";
import oxlintConfig from "../../.oxlintrc.json";

/**
 * Garde-fou contre la régression de la configuration de linting.
 *
 * Voir issue #217 — la règle `typescript/no-explicit-any` doit rester active
 * au niveau `error` dans `.oxlintrc.json` pour préserver la sûreté de type.
 *
 * Ne couvre pas les suppressions locales `// oxlint-disable-next-line` :
 * à la date de cet écrit, aucune n'existe dans `src/`. Si le besoin
 * apparaît, ajouter un test qui grep le tree.
 */
describe(".oxlintrc.json", () => {
  it("should enforce typescript/no-explicit-any as error", () => {
    const rule = oxlintConfig.rules["typescript/no-explicit-any"];
    const severity = Array.isArray(rule) ? rule[0] : rule;
    expect(severity).toBe("error");
  });
});
