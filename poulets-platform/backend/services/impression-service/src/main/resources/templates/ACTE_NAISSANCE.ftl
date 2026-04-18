<#-- Extrait d'Acte de Naissance - Burkina Faso -->
<#-- Template FreeMarker pour generation PDF via Flying Saucer -->
<#-- Converti depuis template Thymeleaf - layout fidele au formulaire officiel -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.0 Strict//EN"
  "http://www.w3.org/TR/xhtml1/DTD/xhtml1-strict.dtd">
<html xmlns="http://www.w3.org/1999/xhtml" lang="fr">
<head>
    <meta charset="UTF-8"/>
    <title>Extrait d'Acte de Naissance</title>
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
            margin-bottom: 20px;
        }

        .title-section h1 {
            margin: 3px 0;
            text-decoration: underline;
            font-size: 18px;
        }

        .title-section p {
            font-size: 14px;
            margin-top: 6px;
        }

        /* ===== CHAMPS ===== */
        .content {
            margin: 10px 10px;
        }

        .info-row {
            margin-bottom: 12px;
            font-size: 13px;
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

        /* ===== PIED DE PAGE (table layout) ===== */
        .footer-table {
            width: 100%;
            border: none;
            border-collapse: collapse;
            margin-top: 25px;
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

        .certify-text {
            font-style: italic;
            font-size: 12px;
            margin-bottom: 15px;
        }

        .signature-name {
            font-weight: bold;
            font-size: 13px;
            margin-top: 35px;
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
        <h1>EXTRAIT D'ACTE DE NAISSANCE</h1>
        <p>
            <strong>N&#176; ${numero!numeroActe!'______'}</strong>
            du ${dateActe!'____/____/________'}
        </p>
    </div>

    <!-- ==================== CHAMPS ==================== -->
    <div class="content">
        <div class="info-row">
            <span class="label">Nom :</span>
            <span class="value">${nom!''}</span>
        </div>

        <div class="info-row">
            <span class="label">Prenom(s) :</span>
            <span class="value">${prenoms!''}</span>
        </div>

        <div class="info-row">
            <span class="label"><#if sexe?? && sexe == 'F'>Nee le<#else>Ne le</#if> :</span>
            <span class="value">${dateNaissanceEnLettres!dateNaissanceLettres!dateNaissance!''}</span>
        </div>

        <div class="info-row">
            <span class="label">A :</span>
            <span class="value">${lieuNaissance!''}</span>
        </div>

        <div class="info-row">
            <span class="label"><#if sexe?? && sexe == 'F'>Fille de<#else>Fils de</#if> :</span>
            <span class="value">${nomPere!pere!''}<#if (professionPere!'')?has_content>, ${professionPere}</#if></span>
        </div>

        <div class="info-row">
            <span class="label">Et de :</span>
            <span class="value">${nomMere!mere!''}<#if (professionMere!'')?has_content>, ${professionMere}</#if></span>
        </div>
    </div>

    <!-- ==================== MENTIONS MARGINALES ==================== -->
    <#if mentionsMarginales?? && (mentionsMarginales?size > 0)>
    <div style="margin: 20px 10px 0 10px; padding: 8px; border-left: 2px solid #000;">
        <div style="font-size: 10px; font-weight: bold; text-transform: uppercase; margin-bottom: 5px;">
            Mentions marginales :
        </div>
        <#list mentionsMarginales as mention>
        <div style="font-size: 11px; font-style: italic; color: #555; margin-bottom: 2px;">
            ${mention}
        </div>
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
                    <img src="${signatureUrl}" style="max-height: 40px; max-width: 120px;" alt="Signature"/>
                    </#if>
                    <div class="signature-name">${nomOfficier!signataire!''}</div>
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
                    <div class="qr-text">Reference: ${referenceUnique!documentReference!'N/A'}</div>
                    <div class="qr-text">ID: ${documentId!'N/A'}</div>
                    <#if (codeVerification!'')?has_content>
                    <div class="qr-text">Code de verification: ${codeVerification}</div>
                    </#if>
                    <#if documentHash?? && documentHash?has_content>
                    <div class="qr-text">Hash: ${documentHash}</div>
                    </#if>
                    <div class="qr-text">Verification: ${verificationUrl!'https://verify.actes.gov.bf'}</div>
                </td>
                <td style="width: 30%; text-align: right;">
                    <div class="qr-text">Scannez le QR code</div>
                    <div class="qr-text">pour verifier l'authenticite</div>
                </td>
            </tr>
        </table>
    </div>
    </#if>

</div>

</body>
</html>
