import { useState, useEffect } from "react";
import { apiRequest, apiUrl } from "@/lib/api";

export type MyDashboardData = {
  // Account info
  accountPlan: "free" | "subscriber" | null;
  walletBalanceEur: number | null;
  
  // Organization info (if applicable)
  organizationName: string | null;
  organizationPlan: "free" | "subscriber" | null;
  organizationWalletBalanceEur: number | null;
  
  // Chat sessions
  recentChatSessions: Array<{
    id: string;
    title: string | null;
    model_id: string | null;
    created_at: string;
    updated_at: string;
  }>;
  totalChatSessions: number;
  
  // Accessible models
  accessibleModels: Array<{
    model: string;
    label: string;
    scope: string;
    underlying_model_id: string;
  }>;
  
  // Token usage (simplified - would need proper endpoint)
  tokensUsed: number | null;
  tokensUsedThisMonth: number | null;
  
  loading: boolean;
  error: string | null;
};

export function useMyDashboard() {
  const [data, setData] = useState<MyDashboardData>({
    accountPlan: null,
    walletBalanceEur: null,
    organizationName: null,
    organizationPlan: null,
    organizationWalletBalanceEur: null,
    recentChatSessions: [],
    totalChatSessions: 0,
    accessibleModels: [],
    tokensUsed: null,
    tokensUsedThisMonth: null,
    loading: true,
    error: null,
  });

  useEffect(() => {
    const fetchData = async () => {
      setData((prev) => ({ ...prev, loading: true, error: null }));
      
      try {
        // Fetch user info (includes account_plan and wallet_balance_eur)
        const meRes = await apiRequest("/auth/me");
        if (!meRes.ok) {
          throw new Error("Failed to fetch user info");
        }
        const meData = await meRes.json();
        
        // Fetch recent chat sessions
        const runsRes = await fetch(apiUrl("workbench/runs?limit=5"), {
          credentials: "include",
        });
        const runsData = runsRes.ok ? await runsRes.json() : [];
        
        // Fetch total count
        const runsAllRes = await fetch(apiUrl("workbench/runs?limit=1000"), {
          credentials: "include",
        });
        const runsAllData = runsAllRes.ok ? await runsAllRes.json() : [];
        
        // Fetch accessible models
        const modelsRes = await fetch(apiUrl("/chat/models"), {
          credentials: "include",
        });
        const modelsData = modelsRes.ok ? await modelsRes.json() : [];
        
        // For now, token usage is not available via API
        // This would need a new endpoint like /auth/me/usage
        
        setData({
          accountPlan: meData.account_plan || "free",
          walletBalanceEur: meData.wallet_balance_eur || null,
          organizationName: meData.current_organization_name || null,
          organizationPlan: meData.current_organization_subscription_plan || null,
          organizationWalletBalanceEur: meData.current_organization_wallet_balance_eur || null,
          recentChatSessions: runsData.slice(0, 5) || [],
          totalChatSessions: runsAllData.length || 0,
          accessibleModels: modelsData || [],
          tokensUsed: null, // TODO: implement endpoint
          tokensUsedThisMonth: null, // TODO: implement endpoint
          loading: false,
          error: null,
        });
      } catch (e) {
        setData((prev) => ({
          ...prev,
          loading: false,
          error: e instanceof Error ? e.message : "Failed to load dashboard data",
        }));
      }
    };

    void fetchData();
  }, []);

  return data;
}

