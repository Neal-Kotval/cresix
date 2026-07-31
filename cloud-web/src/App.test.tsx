import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "./App";

describe("Cresix Cloud preview", () => {
  afterEach(cleanup);

  beforeEach(() => {
    window.history.replaceState({}, "", "/app?preview=1");
  });

  it("labels fixture data and renders repository-style workspaces", () => {
    render(<App />);

    expect(screen.getByText("Preview data")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Workspaces" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "paper-street" })).toBeInTheDocument();
    expect(screen.getAllByText("connected").length).toBeGreaterThan(0);
    expect(screen.getByText("revoked")).toBeInTheDocument();
  });

  it("explains the origin boundary before opening a connected installation", () => {
    window.history.replaceState({}, "", "/paper-street/weeknote?preview=1");
    render(<App />);

    expect(screen.getByRole("heading", { name: "Weeknote" })).toBeInTheDocument();
    expect(screen.getByText(/you are leaving/i)).toBeInTheDocument();
    const open = screen.getByRole("link", { name: /open on c6/i });
    expect(open).toHaveAttribute(
      "href",
      "https://route-7fk2.relay.cresix.com/projects/weeknote",
    );
  });

  it("gives direction when an installation is offline", () => {
    window.history.replaceState({}, "", "/home-lab/garden-watch?preview=1");
    render(<App />);

    expect(screen.getByRole("heading", { name: /installation is offline/i })).toBeInTheDocument();
    expect(screen.queryByRole("link", { name: /open on c6/i })).not.toBeInTheDocument();
  });

  it("does not reveal whether an unknown project or namespace exists", () => {
    window.history.replaceState({}, "", "/private/unknown?preview=1");
    render(<App />);

    expect(screen.getByRole("heading", { name: "Project not found" })).toBeInTheDocument();
  });
});
