import type { QueryClient } from '@tanstack/react-query'
import { createRootRouteWithContext, Link, Outlet, useMatchRoute } from '@tanstack/react-router'
import { Command, Gem, ScanSearch, Settings, Shirt } from 'lucide-react'
import { useTranslation } from 'react-i18next'

import { Badge } from '@/components/ui/badge'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
} from '@/components/ui/sidebar'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { CommandPalette } from '@/features/command-palette'
import { useScan } from '@/features/scan/scan-store'
import { openCommandMenu } from '@/components/ui/command-menu'
import { isDemo } from '@/lib/api'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  component: RootLayout,
})

function RootLayout() {
  const { t } = useTranslation()
  const scan = useScan()

  return (
    <TooltipProvider>
      <SidebarProvider>
        <Sidebar variant="inset">
          <SidebarHeader className="px-3 pt-4">
            <div className="flex items-center gap-2.5">
              <span className="grid size-8 place-items-center rounded-lg bg-primary/15 text-primary ring-1 ring-primary/25">
                <ScanSearch className="size-4" />
              </span>
              <div className="leading-tight">
                <div className="font-semibold tracking-tight">{t('app.name')}</div>
                <div className="text-xs text-muted-foreground">{t('app.region')}</div>
              </div>
            </div>
          </SidebarHeader>
          <SidebarContent>
            <SidebarGroup>
              <SidebarGroupContent>
                <SidebarMenu>
                  <NavItem to="/" icon={<ScanSearch />} label={t('nav.scanner')}>
                    {scan.status === 'running' && (
                      <SidebarMenuBadge className="tabular-nums text-primary">
                        {scan.total ? Math.round((scan.done / scan.total) * 100) : 0}%
                      </SidebarMenuBadge>
                    )}
                  </NavItem>
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
            <SidebarGroup>
              <SidebarGroupLabel>{t('nav.soon')}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {[
                    { icon: <Shirt />, label: t('nav.transmog') },
                    { icon: <Gem />, label: t('nav.collections') },
                  ].map((item) => (
                    <SidebarMenuItem key={item.label}>
                      <SidebarMenuButton disabled className="opacity-50">
                        {item.icon}
                        <span>{item.label}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          </SidebarContent>
          <SidebarFooter>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton onClick={() => openCommandMenu()} className="text-muted-foreground">
                  <Command />
                  <span>{t('command.shortcut')}</span>
                  <kbd className="ml-auto rounded bg-foreground/[0.06] px-1.5 font-mono text-[0.7rem] ring-1 ring-foreground/10">Ctrl K</kbd>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <NavItem to="/settings" icon={<Settings />} label={t('nav.settings')} />
            </SidebarMenu>
            {isDemo && (
              <Badge variant="outline" className="mx-2 mb-2 justify-center border-primary/30 text-primary">
                {t('app.demo')}
              </Badge>
            )}
          </SidebarFooter>
        </Sidebar>
        <SidebarInset className="min-w-0">
          <Outlet />
        </SidebarInset>
      </SidebarProvider>
      <CommandPalette />
      <Toaster theme="dark" position="bottom-right" />
    </TooltipProvider>
  )
}

function NavItem({
  to,
  icon,
  label,
  children,
}: {
  to: '/' | '/settings'
  icon: React.ReactNode
  label: string
  children?: React.ReactNode
}) {
  const matchRoute = useMatchRoute()
  return (
    <SidebarMenuItem>
      <SidebarMenuButton asChild isActive={Boolean(matchRoute({ to }))}>
        <Link to={to}>
          {icon}
          <span>{label}</span>
        </Link>
      </SidebarMenuButton>
      {children}
    </SidebarMenuItem>
  )
}
