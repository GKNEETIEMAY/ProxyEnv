/** Selection revisions do not change on environment writes; invalidate pre-write reads too. */
export class RefreshEpoch {
  private generation = 0;
  capture() { return this.generation; }
  invalidate() { this.generation++; }
  accepts(generation: number) { return generation === this.generation; }
}
