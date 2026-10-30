import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { CircleCheck, CircleDashed, ExternalLink, Search } from 'lucide-react'
import { useMemo, useState, type FormEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import type { Language } from '@/i18n'
import { api } from '@/lib/api'
import { useErrorMessage } from '@/lib/errors'
import { realmName, useApiLocale } from '@/lib/format'
import { openUrl } from '@/lib/open-url'
import { type PreferredVersion, settings, useSettings } from '@/lib/settings'

const DEVELOPER_PORTAL = 'https://develop.battle.net/access/clients'

export function SettingsPage() {
  const { t } = useTranslation()
  return (
    <div className="mx-auto w-full max-w-3xl space-y-6 p-6">
      <h1 className="text-2xl font-semibold tracking-tight">{t('settings.title')}</h1>
      <CredentialsSection />
      <LanguageSection />
      <VersionSection />
      <RealmsSection />
      <NewRealmCostSection />
    </div>
  )
}

function Section({ title, description, aside, children }: { title: string; description: string; aside?: ReactNode; children: ReactNode }) {
  return (
    <section className="space-y-4 rounded-xl bg-card p-5 ring-1 ring-foreground/8">
      <div className="flex items-start gap-4">
        <div className="flex-1 space-y-1">
          <h2 className="font-semibold tracking-tight">{title}</h2>
          <p className="text-sm text-muted-foreground">{description}</p>
        </div>
        {aside}
      </div>
      {children}
    </section>
  )
}

function CredentialsSection() {
  const { t } = useTranslation()
  const errorMessage = useErrorMessage()
  const queryClient = useQueryClient()
  const [clientId, setClientId] = useState('')
  const [clientSecret, setClientSecret] = useState('')
  const configured = useQuery({ queryKey: ['credentials'], queryFn: api.hasCredentials })

  const refresh = () => queryClient.invalidateQueries()
  const save = useMutation({
    mutationFn: () => api.saveCredentials(clientId, clientSecret),
    onSuccess: () => {
      setClientId('')
      setClientSecret('')
      toast.success(t('settings.credentials.saved'))
      void refresh()
    },
    onError: (error) => toast.error(errorMessage(error)),
  })
  const remove = useMutation({ mutationFn: api.clearCredentials, onSuccess: () => void refresh() })

  function onSubmit(event: FormEvent) {
    event.preventDefault()
    if (clientId.trim() && clientSecret.trim()) save.mutate()
  }

  return (
    <Section
      title={t('settings.credentials.title')}
      description={t('settings.credentials.description')}
      aside={
        configured.data ? (
          <Badge variant="outline" className="border-primary/30 text-primary">
            <CircleCheck /> {t('settings.credentials.configured')}
          </Badge>
        ) : (
          <Badge variant="outline" className="text-muted-foreground">
            <CircleDashed /> {t('settings.credentials.missing')}
          </Badge>
        )
      }
    >
      <form onSubmit={onSubmit} className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-1.5">
          <Label htmlFor="client-id">{t('settings.credentials.clientId')}</Label>
          <Input id="client-id" value={clientId} onChange={(e) => setClientId(e.target.value)} autoComplete="off" spellCheck={false} />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="client-secret">{t('settings.credentials.clientSecret')}</Label>
          <Input id="client-secret" type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} autoComplete="off" />
        </div>
        <div className="flex flex-wrap items-center gap-2 sm:col-span-2">
          <Button type="submit" disabled={save.isPending || !clientId.trim() || !clientSecret.trim()}>
            {save.isPending ? t('settings.credentials.saving') : t('settings.credentials.save')}
          </Button>
          <Button type="button" variant="ghost" onClick={() => openUrl(DEVELOPER_PORTAL)}>
            <ExternalLink /> {t('settings.credentials.open')}
          </Button>
          {configured.data && (
            <Button type="button" variant="ghost" className="ml-auto text-muted-foreground" onClick={() => remove.mutate()}>
              {t('settings.credentials.remove')}
            </Button>
          )}
        </div>
        <p className="text-xs text-muted-foreground sm:col-span-2">{t('settings.credentials.storage')}</p>
      </form>
    </Section>
  )
}

function LanguageSection() {
  const { t } = useTranslation()
  const { language } = useSettings()
  return (
    <Section
      title={t('settings.language.title')}
      description={t('settings.language.description')}
      aside={
        <Select value={language} onValueChange={(value) => settings.update({ language: value as Language })}>
          <SelectTrigger className="w-36">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="fr">Français</SelectItem>
            <SelectItem value="en">English</SelectItem>
          </SelectContent>
        </Select>
      }
    >
      {null}
    </Section>
  )
}

function VersionSection() {
  const { t } = useTranslation()
  const { preferredVersion } = useSettings()
  const choices: PreferredVersion[] = ['cheapest', 'mythic', 'heroic', 'normal', 'raidFinder']
  return (
    <Section
      title={t('settings.version.title')}
      description={t('settings.version.description')}
      aside={
        <Select value={preferredVersion} onValueChange={(value) => settings.update({ preferredVersion: value as PreferredVersion })}>
          <SelectTrigger className="w-52">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {choices.map((choice) => (
              <SelectItem key={choice} value={choice}>
                {t(`settings.version.${choice}`)}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      }
    >
      {null}
    </Section>
  )
}

function RealmsSection() {
  const { t } = useTranslation()
  const locale = useApiLocale()
  const { ownedRealms } = useSettings()
  const [search, setSearch] = useState('')
  const configured = useQuery({ queryKey: ['credentials'], queryFn: api.hasCredentials })
  const realms = useQuery({ queryKey: ['realms'], queryFn: api.connectedRealms, enabled: configured.data === true })

  const owned = new Set(ownedRealms)
  const list = useMemo(() => {
    const needle = search.trim().toLocaleLowerCase()
    return (realms.data ?? [])
      .map((r) => ({ id: r.id, name: realmName(r, locale) }))
      .filter((r) => !needle || r.name.toLocaleLowerCase().includes(needle))
      .sort((a, b) => a.name.localeCompare(b.name))
  }, [realms.data, search, locale])

  function toggle(id: number, checked: boolean) {
    settings.update({ ownedRealms: checked ? [...ownedRealms, id] : ownedRealms.filter((r) => r !== id) })
  }

  return (
    <Section
      title={t('settings.realms.title')}
      description={t('settings.realms.description')}
      aside={<Badge variant="secondary" className="tabular-nums">{t('settings.realms.selected', { count: ownedRealms.length })}</Badge>}
    >
      {configured.data !== true ? null : realms.isPending ? (
        <p className="text-sm text-muted-foreground">{t('settings.realms.loading')}</p>
      ) : (
        <div className="space-y-2">
          <div className="relative">
            <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t('settings.realms.search')} className="pl-8" />
          </div>
          <ScrollArea className="h-72 rounded-lg ring-1 ring-foreground/8">
            <ul className="p-1">
              {list.map((realm) => (
                <li key={realm.id}>
                  <label className="flex cursor-pointer items-center gap-3 rounded-md px-2.5 py-2 text-sm transition-colors duration-150 hover:bg-foreground/[0.04]">
                    <Checkbox checked={owned.has(realm.id)} onCheckedChange={(c) => toggle(realm.id, c === true)} />
                    <span className="min-w-0 truncate">{realm.name}</span>
                  </label>
                </li>
              ))}
            </ul>
          </ScrollArea>
        </div>
      )}
    </Section>
  )
}

function NewRealmCostSection() {
  const { t } = useTranslation()
  const { newRealmCostGold } = useSettings()
  return (
    <Section
      title={t('settings.newRealmCost.title')}
      description={t('settings.newRealmCost.description')}
      aside={
        <div className="flex items-center gap-2">
          <Input
            type="number"
            min={0}
            step={100}
            value={newRealmCostGold}
            onChange={(e) => settings.update({ newRealmCostGold: Math.max(0, Math.floor(Number(e.target.value) || 0)) })}
            className="w-28 text-right tabular-nums"
          />
          <span className="text-sm text-muted-foreground">{t('settings.newRealmCost.unit')}</span>
        </div>
      }
    >
      {null}
    </Section>
  )
}
