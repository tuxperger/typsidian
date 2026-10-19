// Воркер силовой раскладки графа (фича 025, research.md §6).
//
// Главный поток только рисует: здесь считается d3-force, а положения уходят
// массивом `Float32Array` (`x0, y0, x1, y1, …`) в порядке узлов графа. Сообщения
// шлются не чаще кадра, чтобы очередь главного потока не росла.

import {
  forceLink,
  forceSimulation,
  forceX,
  forceY,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import type { FromWorker, LayoutForces, ToWorker } from "./layout-protocol.js";
import { forceRepel, type RepelForce } from "./repel-force.js";

interface WorkerScope {
  onmessage: ((event: MessageEvent<ToWorker>) => void) | null;
  postMessage(message: FromWorker, transfer: Transferable[]): void;
  postMessage(message: FromWorker): void;
}

const scope = self as unknown as WorkerScope;

interface Body extends SimulationNodeDatum {
  id: string;
  degree: number;
}
type Tie = SimulationLinkDatum<Body>;

/** Покой не позже: после этого времени от последнего подогрева раскладка замирает. */
const MAX_RUN_MS = 7000;
const FRAME_MS = 16;
const KNOWN_LIMIT = 60000;

let forces: LayoutForces = { center: 0.3, repel: 0.6, link: 0.5, distance: 0.45 };
let sim: Simulation<Body, Tie> | null = null;
let bodies: Body[] = [];
let ties: Tie[] = [];
let gen = 0;
let timer: ReturnType<typeof setTimeout> | null = null;
let runStarted = 0;
let lastPost = 0;
let settledSent = true;
/** Последние известные положения по id: переживают смену графа (research.md §8). */
const known = new Map<string, { x: number; y: number }>();

const repelStrength = (): number => -(12 + 190 * forces.repel);
const centerStrength = (): number => 0.01 + 0.13 * forces.center;
const linkDistance = (): number => 12 + 110 * forces.distance;
const linkScale = (): number => 0.2 + 1.6 * forces.link;

function remember(): void {
  for (const body of bodies) {
    if (body.x !== undefined && body.y !== undefined) {
      known.set(body.id, { x: body.x, y: body.y });
    }
  }
  if (known.size > KNOWN_LIMIT) {
    const keep = new Set(bodies.map((body) => body.id));
    for (const id of known.keys()) {
      if (known.size <= KNOWN_LIMIT / 2) break;
      if (!keep.has(id)) known.delete(id);
    }
  }
}

function applyForces(): void {
  if (sim === null) return;
  const many = sim.force("charge") as RepelForce<Body> | undefined;
  many?.strength(repelStrength());
  const link = sim.force("link") as ReturnType<typeof forceLink<Body, Tie>> | undefined;
  link?.distance(linkDistance()).strength((tie) => {
    const a = tie.source as Body;
    const b = tie.target as Body;
    return Math.min(1, linkScale() / Math.max(1, Math.min(a.degree, b.degree)));
  });
  (sim.force("x") as ReturnType<typeof forceX<Body>> | undefined)?.strength(centerStrength());
  (sim.force("y") as ReturnType<typeof forceY<Body>> | undefined)?.strength(centerStrength());
}

function post(force: boolean): void {
  if (sim === null) return;
  const now = performance.now();
  if (!force && now - lastPost < FRAME_MS) return;
  lastPost = now;
  const positions = new Float32Array(bodies.length * 2);
  for (let i = 0; i < bodies.length; i += 1) {
    const body = bodies[i]!;
    positions[i * 2] = body.x ?? 0;
    positions[i * 2 + 1] = body.y ?? 0;
  }
  scope.postMessage({ type: "tick", gen, positions, alpha: sim.alpha() }, [positions.buffer]);
}

function loop(): void {
  timer = null;
  if (sim === null) return;
  const started = performance.now();
  // Тик за тиком, пока не уйдёт кадровый бюджет: на малом графе это много тиков
  // за кадр, на большом — один.
  do {
    sim.tick();
  } while (performance.now() - started < FRAME_MS / 2 && sim.alpha() > sim.alphaMin());

  const expired = performance.now() - runStarted > MAX_RUN_MS;
  const calm = sim.alpha() <= sim.alphaMin() && sim.alphaTarget() === 0;
  if (expired && sim.alphaTarget() === 0) {
    sim.alpha(0);
  }
  post(calm || expired);
  if (calm || (expired && sim.alphaTarget() === 0)) {
    remember();
    if (!settledSent) {
      settledSent = true;
      scope.postMessage({ type: "settled", gen });
    }
    return;
  }
  timer = setTimeout(loop, 0);
}

function wake(alpha: number): void {
  if (sim === null) return;
  sim.alpha(Math.max(sim.alpha(), alpha));
  runStarted = performance.now();
  settledSent = false;
  if (timer === null) timer = setTimeout(loop, 0);
}

function setGraph(message: Extract<ToWorker, { type: "setGraph" }>): void {
  remember();
  for (const [from, to] of message.renames) {
    const position = known.get(from);
    if (position !== undefined) known.set(to, position);
  }
  sim?.stop();
  if (timer !== null) clearTimeout(timer);
  timer = null;

  const degree = new Array<number>(message.ids.length).fill(0);
  const edges = message.edges;
  for (let i = 0; i < edges.length; i += 2) {
    degree[edges[i]!]! += 1;
    degree[edges[i + 1]!]! += 1;
  }

  gen = message.gen;
  bodies = message.ids.map((id, index) => {
    const body: Body = { id, degree: degree[index] ?? 0 };
    const at = known.get(id);
    if (at !== undefined) {
      body.x = at.x;
      body.y = at.y;
    }
    return body;
  });

  // Новый узел появляется рядом с уже размещённым соседом (research.md §8).
  const placed = new Set<number>();
  bodies.forEach((body, index) => {
    if (body.x !== undefined) placed.add(index);
  });
  if (placed.size > 0 && placed.size < bodies.length) {
    for (let pass = 0; pass < 2; pass += 1) {
      for (let i = 0; i < edges.length; i += 2) {
        const a = edges[i]!;
        const b = edges[i + 1]!;
        const bodyA = bodies[a]!;
        const bodyB = bodies[b]!;
        for (const [from, to, target] of [
          [bodyA, bodyB, b],
          [bodyB, bodyA, a],
        ] as const) {
          if (from.x !== undefined && to.x === undefined) {
            to.x = from.x + (Math.random() - 0.5) * 20;
            to.y = (from.y ?? 0) + (Math.random() - 0.5) * 20;
            placed.add(target);
          }
        }
      }
    }
  }
  // Центр локального графа без положения — в начало координат.
  if (message.center !== null) {
    const centre = bodies[message.center];
    if (centre !== undefined && centre.x === undefined) {
      centre.x = 0;
      centre.y = 0;
    }
  }

  ties = [];
  for (let i = 0; i < edges.length; i += 2) {
    ties.push({ source: edges[i]!, target: edges[i + 1]! });
  }

  const fresh = placed.size === 0;
  sim = forceSimulation<Body>(bodies)
    .alphaDecay(bodies.length > 2000 ? 0.05 : 0.03)
    .velocityDecay(0.4)
    .force("charge", forceRepel<Body>().theta(1.1).distanceMax(300))
    .force("link", forceLink<Body, Tie>(ties).iterations(1))
    .force("x", forceX<Body>(0))
    .force("y", forceY<Body>(0))
    .stop();
  applyForces();
  // Первый граф раскладывается с полной энергией, обновление — слегка подогревает.
  sim.alpha(fresh ? 1 : 0.1);
  runStarted = performance.now();
  settledSent = false;
  post(true);
  timer = setTimeout(loop, 0);
}

scope.onmessage = (event) => {
  const message = event.data;
  switch (message.type) {
    case "setGraph":
      setGraph(message);
      return;
    case "setForces":
      forces = message.forces;
      applyForces();
      wake(0.3);
      return;
    case "drag": {
      const body = bodies[message.index];
      if (body === undefined || sim === null) return;
      body.fx = message.x;
      body.fy = message.y;
      sim.alphaTarget(0.3);
      wake(0.3);
      return;
    }
    case "release": {
      for (const body of bodies) {
        body.fx = null;
        body.fy = null;
      }
      sim?.alphaTarget(0);
      wake(0.05);
      return;
    }
    case "reheat":
      wake(message.alpha);
      return;
    case "stop":
      sim?.stop();
      if (timer !== null) clearTimeout(timer);
      timer = null;
      return;
    default: {
      const exhaustive: never = message;
      return exhaustive;
    }
  }
};
