<#-- Extrait d'Acte de Deces - Burkina Faso -->
<#-- Template FreeMarker pour generation PDF via Flying Saucer -->
<#-- Layout fidele au formulaire officiel - table-based (Flying Saucer compatible) -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="fr">
<head>
    <meta charset="UTF-8"/>
    <title>Extrait d'Acte de D&#233;c&#232;s</title>
    <style type="text/css">
        @page {
            size: A4;
            margin: 15mm 15mm 15mm 15mm;
        }

        body {
            font-family: 'Times New Roman', Times, serif;
            margin: 0;
            padding: 0;
            color: #000;
            font-size: 12px;
        }

        .container {
            border: 2px solid #000;
            padding: 15px;
        }

        /* ===== EN-TETE (table layout pour Flying Saucer) ===== */
        .header-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-bottom: 15px;
        }

        .header-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .header-left {
            text-align: left;
            font-weight: bold;
            font-size: 12px;
            line-height: 1.6;
        }

        .header-right {
            text-align: center;
            font-size: 13px;
        }

        .header-right strong {
            font-size: 14px;
        }

        .motto {
            font-style: italic;
            font-size: 11px;
        }

        /* ===== TITRE ===== */
        .title-section {
            text-align: center;
            margin-bottom: 15px;
        }

        .title-section h1 {
            margin: 3px 0;
            text-decoration: underline;
            font-size: 17px;
        }

        .title-section h2 {
            margin: 3px 0;
            text-decoration: underline;
            font-size: 15px;
        }

        .title-section p {
            font-size: 13px;
            margin-top: 6px;
        }

        /* ===== CHAMPS ===== */
        .content {
            margin: 10px 10px;
        }

        .info-row {
            margin-bottom: 10px;
            font-size: 12px;
            line-height: 1.5;
        }

        .label {
            font-weight: bold;
            text-transform: uppercase;
        }

        .value {
            border-bottom: 1px dotted #000;
            padding-left: 10px;
            padding-bottom: 2px;
        }

        /* ===== SECTION SEPARATEUR ===== */
        .section-title {
            font-weight: bold;
            font-size: 12px;
            text-decoration: underline;
            margin-top: 12px;
            margin-bottom: 6px;
            text-transform: uppercase;
        }

        /* ===== MENTIONS MARGINALES ===== */
        .mentions-section {
            margin-top: 20px;
            border: 1px dashed #666;
            padding: 10px;
        }

        .mentions-title {
            font-weight: bold;
            font-size: 12px;
            text-decoration: underline;
            margin-bottom: 8px;
        }

        .mention-item {
            font-size: 11px;
            margin-bottom: 4px;
            padding-left: 10px;
        }

        /* ===== CERTIFICATION ===== */
        .certify-text {
            font-style: italic;
            font-size: 12px;
            margin-bottom: 15px;
        }

        /* ===== PIED DE PAGE (table layout) ===== */
        .footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-top: 20px;
        }

        .footer-table td {
            border: none;
            vertical-align: top;
            padding: 0;
        }

        .stamp-box {
            border: 1px solid #000;
            width: 150px;
            height: 80px;
            text-align: center;
            font-size: 11px;
            padding-top: 20px;
        }

        .signature-section {
            text-align: center;
        }

        .signature-name {
            font-weight: bold;
            font-size: 13px;
            margin-top: 30px;
        }

        /* ===== QR CODE VERIFICATION ===== */
        .qr-footer {
            margin-top: 30px;
            border-top: 1px solid #ccc;
            padding-top: 8px;
        }

        .qr-footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
        }

        .qr-footer-table td {
            border: none;
            vertical-align: middle;
            padding: 3px;
        }

        .qr-text {
            font-size: 8px;
            color: #666;
        }

        .qr-text-bold {
            font-size: 8px;
            color: #666;
            font-weight: bold;
        }
    </style>
</head>
<body>

<div class="container">

    <!-- ==================== EN-TETE ==================== -->
    <table class="header-table">
        <tr>
            <td class="header-left" style="width: 45%;">
                <div>${region!'REGION DU CENTRE'}</div>
                <div>${province!'PROVINCE DU KADIOGO'}</div>
                <div>COMMUNE DE ${commune!'OUAGADOUGOU'}</div>
            </td>
            <td class="header-right" style="width: 55%;">
                <strong>BURKINA FASO</strong><br/>
                <span class="motto">Unit&#233; &#8212; Progr&#232;s &#8212; Justice</span>
            </td>
        </tr>
    </table>

    <!-- ==================== TITRE ==================== -->
    <div class="title-section">
        <h1>ETAT CIVIL</h1>
        <h2>EXTRAIT D'ACTE DE D&#201;C&#200;S</h2>
        <p>
            <strong>N&#176; ${numero!'______'}</strong>
            du ${dateActe!'____/____/________'}
        </p>
    </div>

    <!-- ==================== IDENTITE DU DEFUNT ==================== -->
    <div class="content">
        <div class="section-title">Identit&#233; du d&#233;funt</div>

        <div class="info-row">
            <span class="label">Nom :</span>
            <span class="value">${nomDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Prenom(s) :</span>
            <span class="value">${prenomsDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Sexe :</span>
            <span class="value">${sexeDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label"><#if sexeDefunt?? && sexeDefunt == 'F'>N&#233;e le<#else>N&#233; le</#if> :</span>
            <span class="value">${dateNaissanceDefuntEnLettres!dateNaissanceDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label">A :</span>
            <span class="value">${lieuNaissanceDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label"><#if sexeDefunt?? && sexeDefunt == 'F'>Fille de<#else>Fils de</#if> :</span>
            <span class="value">${nomPereDefunt!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Et de :</span>
            <span class="value">${nomMereDefunt!''}</span>
        </div>

        <#if professionDefunt?? && professionDefunt?has_content>
        <div class="info-row">
            <span class="label">Profession :</span>
            <span class="value">${professionDefunt}</span>
        </div>
        </#if>

        <#if domicileDefunt?? && domicileDefunt?has_content>
        <div class="info-row">
            <span class="label">Domicile :</span>
            <span class="value">${domicileDefunt}</span>
        </div>
        </#if>

        <#if situationMatrimoniale?? && situationMatrimoniale?has_content>
        <div class="info-row">
            <span class="label">Situation matrimoniale :</span>
            <span class="value">${situationMatrimoniale}</span>
        </div>
        </#if>

        <div class="info-row">
            <span class="label">Nationalite :</span>
            <span class="value">${nationaliteDefunt!'Burkinab\u00e8'}</span>
        </div>

        <!-- ==================== INFORMATIONS SUR LE DECES ==================== -->
        <div class="section-title">Circonstances du d&#233;c&#232;s</div>

        <div class="info-row">
            <span class="label"><#if sexeDefunt?? && sexeDefunt == 'F'>D&#233;c&#233;d&#233;e le<#else>D&#233;c&#233;d&#233; le</#if> :</span>
            <span class="value">${dateDecesEnLettres!dateDeces!''}</span>
        </div>

        <div class="info-row">
            <span class="label">A :</span>
            <span class="value">${lieuDeces!''}</span>
        </div>

        <#if heureDeces?? && heureDeces?has_content>
        <div class="info-row">
            <span class="label">A l'heure de :</span>
            <span class="value">${heureDeces}</span>
        </div>
        </#if>

        <#if causeDeces?? && causeDeces?has_content>
        <div class="info-row">
            <span class="label">Cause du deces :</span>
            <span class="value">${causeDeces}</span>
        </div>
        </#if>

        <!-- ==================== CONSTATATION MEDICALE ==================== -->
        <#if (medecinCertificateur?? && medecinCertificateur?has_content) || (numeroCertificatMedical?? && numeroCertificatMedical?has_content)>
        <div class="section-title">Constatation m&#233;dicale</div>

        <#if medecinCertificateur?? && medecinCertificateur?has_content>
        <div class="info-row">
            <span class="label">Medecin certificateur :</span>
            <span class="value">${medecinCertificateur}</span>
        </div>
        </#if>

        <#if numeroCertificatMedical?? && numeroCertificatMedical?has_content>
        <div class="info-row">
            <span class="label">N&#176; certificat medical :</span>
            <span class="value">${numeroCertificatMedical}</span>
            <#if dateCertificatMedical??>
            <span> du ${dateCertificatMedical}</span>
            </#if>
        </div>
        </#if>

        <#if etablissementSante?? && etablissementSante?has_content>
        <div class="info-row">
            <span class="label">Etablissement de sante :</span>
            <span class="value">${etablissementSante}</span>
        </div>
        </#if>
        </#if>

        <!-- ==================== DECLARANT ==================== -->
        <#if nomDeclarant?? && nomDeclarant?has_content>
        <div class="section-title">D&#233;clarant</div>

        <div class="info-row">
            <span class="label">Nom et prenoms :</span>
            <span class="value">${nomDeclarant}</span>
        </div>

        <#if lienDeclarant?? && lienDeclarant?has_content>
        <div class="info-row">
            <span class="label">Qualite / Lien :</span>
            <span class="value">${lienDeclarant}</span>
        </div>
        </#if>

        <#if adresseDeclarant?? && adresseDeclarant?has_content>
        <div class="info-row">
            <span class="label">Adresse :</span>
            <span class="value">${adresseDeclarant}</span>
        </div>
        </#if>
        </#if>
    </div>

    <!-- ==================== MENTIONS MARGINALES ==================== -->
    <#if mentionsMarginales?? && mentionsMarginales?size gt 0>
    <div class="mentions-section">
        <div class="mentions-title">Mentions marginales</div>
        <#list mentionsMarginales as mention>
        <div class="mention-item">- ${mention}</div>
        </#list>
    </div>
    </#if>

    <!-- ==================== PIED DE PAGE ==================== -->
    <table class="footer-table">
        <tr>
            <td style="width: 180px; vertical-align: top;">
                <div class="stamp-box">
                    TIMBRE D'ETAT CIVIL<br/>
                    300 FRANCS
                </div>
            </td>
            <td style="padding-left: 30px;">
                <div class="signature-section">
                    <div class="certify-text">
                        Certifie le present extrait conforme aux indications portees sur le registre.
                    </div>
                    <p>Delivre a ${lieuDelivrance!'Ouagadougou'}, le ${dateDelivrance!'____/____/________'}</p>
                    <p><strong>L'Officier de l'Etat Civil Delegue,</strong></p>
                    <#if signatureUrl?? && signatureUrl?has_content>
                    <img src="${signatureUrl}" alt="Signature" style="max-width: 200px; max-height: 60px;"/>
                    </#if>
                    <div class="signature-name">${nomOfficier!''}</div>
                </div>
            </td>
        </tr>
    </table>

    <!-- ==================== QR CODE VERIFICATION ==================== -->
    <#if referenceUnique?? || codeVerification?? || qrCodeUrl??>
    <div class="qr-footer">
        <table class="qr-footer-table">
            <tr>
                <td style="width: 70%;">
                    <div class="qr-text-bold">DOCUMENT OFFICIEL SECURISE</div>
                    <#if referenceUnique?? && referenceUnique?has_content>
                    <div class="qr-text">Reference: ${referenceUnique}</div>
                    </#if>
                    <#if codeVerification?? && codeVerification?has_content>
                    <div class="qr-text">Code de verification: ${codeVerification}</div>
                    </#if>
                    <#if documentHash?? && documentHash?has_content>
                    <div style="font-size: 6pt; color: #666; margin-top: 2px;">Hash: ${documentHash}</div>
                    </#if>
                    <div class="qr-text">Verification: ${verificationUrl!'https://verify.actes.gov.bf'}</div>
                </td>
                <td style="width: 30%; text-align: right;">
                    <#if qrCodeUrl?? && qrCodeUrl?has_content>
                    <img src="${qrCodeUrl}" alt="QR Code" style="width: 80px; height: 80px;"/>
                    <#else>
                    <div class="qr-text">Scannez le QR code</div>
                    <div class="qr-text">pour verifier l'authenticite</div>
                    </#if>
                </td>
            </tr>
        </table>
    </div>
    </#if>

</div>

</body>
</html>
