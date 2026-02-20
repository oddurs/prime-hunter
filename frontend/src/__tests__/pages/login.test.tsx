import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import LoginPage from "@/app/login/page";

const mockSignIn = vi.fn();

vi.mock("@/contexts/auth-context", () => ({
  useAuth: () => ({
    signIn: mockSignIn,
    signOut: vi.fn(),
    user: null,
    session: null,
    loading: false,
  }),
}));

describe("LoginPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSignIn.mockResolvedValue(null);
  });

  it("renders login form", () => {
    render(<LoginPage />);
    expect(screen.getByText("darkreach")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("you@example.com")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("Password")).toBeInTheDocument();
    expect(screen.getByText("Sign in")).toBeInTheDocument();
  });

  it("submits form with email and password", async () => {
    const user = userEvent.setup();
    render(<LoginPage />);

    await user.type(screen.getByPlaceholderText("you@example.com"), "test@example.com");
    await user.type(screen.getByPlaceholderText("Password"), "secret123");
    await user.click(screen.getByText("Sign in"));

    expect(mockSignIn).toHaveBeenCalledWith("test@example.com", "secret123");
  });

  it("shows error message on failed sign-in", async () => {
    mockSignIn.mockResolvedValue("Invalid credentials");
    const user = userEvent.setup();
    render(<LoginPage />);

    await user.type(screen.getByPlaceholderText("you@example.com"), "test@example.com");
    await user.type(screen.getByPlaceholderText("Password"), "wrong");
    await user.click(screen.getByText("Sign in"));

    expect(await screen.findByText("Invalid credentials")).toBeInTheDocument();
  });

  it("disables button while loading", async () => {
    // Make signIn never resolve to keep loading state
    mockSignIn.mockReturnValue(new Promise(() => {}));
    const user = userEvent.setup();
    render(<LoginPage />);

    await user.type(screen.getByPlaceholderText("you@example.com"), "test@example.com");
    await user.type(screen.getByPlaceholderText("Password"), "pass");
    await user.click(screen.getByText("Sign in"));

    expect(screen.getByText("Signing in...")).toBeInTheDocument();
  });
});
