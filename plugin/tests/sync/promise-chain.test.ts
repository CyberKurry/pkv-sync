import { describe, expect, it, vi } from "vitest";
import { PromiseChain } from "../../src/util/promise-chain";

/**
 * T-1: the shared FIFO mutex extracted from three hand-rolled chains
 * (plugin-store writeChain, connection-context runLocal, engine
 * eventChain). Pins the two properties every site relies on.
 */
describe("PromiseChain", () => {
  it("runs tasks in submission order without interleaving", async () => {
    const chain = new PromiseChain();
    const order: string[] = [];
    const gate = new Promise<void>((resolve) => setTimeout(resolve, 10));

    const first = chain.enqueue(async () => {
      order.push("first-start");
      await gate;
      order.push("first-end");
    });
    const second = chain.enqueue(async () => {
      order.push("second-start");
    });

    await Promise.all([first, second]);
    expect(order).toEqual(["first-start", "first-end", "second-start"]);
  });

  it("a rejected task does not poison later tasks", async () => {
    const chain = new PromiseChain();
    const boom = chain.enqueue(async () => {
      throw new Error("boom");
    });
    await expect(boom).rejects.toThrow("boom");

    const after = vi.fn(async () => "ok");
    await expect(chain.enqueue(after)).resolves.toBe("ok");
    expect(after).toHaveBeenCalledTimes(1);
  });
});
