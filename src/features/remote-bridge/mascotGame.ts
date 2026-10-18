// Decorative, autonomous games: no keyboard capture, network, or persistent state.
export type GameInput = "left" | "right" | "up" | "down" | "action";
export type GameMode = "duel" | "snake" | "jump";
export type GameFrame = {
  mode: GameMode;
  tick: number;
  health: [number, number];
  attacker: number;
  damaged: number | null;
  damage: number;
  inputs: [GameInput, GameInput];
  head: number;
  length: number;
  food: number;
  score: number;
};
type Random = () => number;
const pick = (random: Random, count: number) => Math.min(count - 1, Math.floor(random() * count));

// A continuous, non-intersecting route lets the little snake visibly collect food.
export const snakeRoute: ReadonlyArray<readonly [number, number]> = [
  ...Array.from({ length: 14 }, (_, x) => [x + 1, 1] as const),
  ...Array.from({ length: 7 }, (_, row) => Array.from({ length: 13 }, (_, column) =>
    [row % 2 ? column + 2 : 14 - column, row + 2] as const)).flat(),
  ...Array.from({ length: 7 }, (_, row) => [1, 8 - row] as const),
];

function nextFood(head: number, random: Random) {
  return (head + 3 + pick(random, 5)) % snakeRoute.length;
}

export function createGame(duo: boolean, random: Random = Math.random): GameFrame {
  return {
    mode: duo ? "duel" : pick(random, 2) ? "jump" : "snake",
    tick: 0, health: [100, 100], attacker: pick(random, 2), damaged: null, damage: 0,
    inputs: ["right", "left"], head: 2, length: 3, food: nextFood(2, random), score: 0,
  };
}

export function snakeCells(frame: GameFrame) {
  return Array.from({ length: frame.length }, (_, offset) =>
    snakeRoute[(frame.head - offset + snakeRoute.length) % snakeRoute.length]!);
}

export function advanceGame(previous: GameFrame, random: Random = Math.random): GameFrame {
  const frame: GameFrame = { ...previous, tick: previous.tick + 1, health: [...previous.health],
    inputs: [...previous.inputs], damaged: null, damage: 0 };
  if (frame.mode === "duel") {
    if (frame.health.includes(0)) return frame; // Let the winner celebrate until this scene ends.
    const phase = frame.tick % 3;
    if (phase === 1) frame.attacker = pick(random, 2);
    frame.inputs = phase === 1 || phase === 2
      ? frame.attacker === 0 ? ["action", "left"] : ["right", "action"]
      : ["right", "left"];
    if (phase === 2) {
      const defender = frame.attacker === 0 ? 1 : 0;
      frame.damage = Math.min(frame.health[defender], 9 + pick(random, 15));
      frame.health[defender] -= frame.damage;
      frame.damaged = defender;
    }
  } else if (frame.mode === "snake") {
    const before = snakeRoute[frame.head]!;
    frame.head = (frame.head + 1) % snakeRoute.length;
    const after = snakeRoute[frame.head]!;
    frame.inputs[0] = after[0] > before[0] ? "right" : after[0] < before[0] ? "left"
      : after[1] > before[1] ? "down" : "up";
    if (frame.head === frame.food) {
      frame.length = Math.min(10, frame.length + 1);
      frame.score += 1;
      frame.food = nextFood(frame.head, random);
    }
  } else {
    const phase = frame.tick % 8;
    frame.inputs[0] = phase >= 2 && phase <= 5 ? "action" : "right";
    if (phase === 4) frame.score += 1;
  }
  return frame;
}

export function jumpPose(tick: number) {
  const phase = tick % 8;
  return { x: 16 + phase * 9, y: 58 - [0, 0, 12, 24, 28, 24, 12, 0][phase]! };
}
