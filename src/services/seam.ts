// §11.1 step 3 — the seam as a gauge: how much of the library answered, as a
// length below the well. Log, so 3 and 30 both read; floored, so one answer is
// never a dark seam. Quantity only: an exact name hit does not light it all.

export const SEAM_FLOOR = 0.1;

/** The answered rest's length, 0–1 of the seam below the well. 0: no gauge. */
export function seamReach(answered: number, library: number): number {
  if (answered <= 0) return 0;
  const total = Math.max(library, answered, 1);
  if (total === 1) return 1;
  const share = Math.log1p(answered) / Math.log1p(total);
  return Math.round((SEAM_FLOOR + (1 - SEAM_FLOOR) * share) * 1000) / 1000;
}
